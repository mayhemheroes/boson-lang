extern crate byteorder;

use byteorder::BigEndian;
use byteorder::LittleEndian;
use byteorder::WriteBytesExt;

use crate::compiler::CompiledBytecode;
use crate::compiler::CompiledInstructions;
use crate::types::object::Object;

use std::collections::HashMap;
use std::fs;
use std::mem;
use std::slice;

const USE_BIG_ENDIAN_REPR: bool = false;
const MAGIC: &str = "000BOSON";

#[allow(dead_code)]
#[repr(u8)]
pub enum TypeCode {
    NONE,
    CHAR,
    INT,
    STR,
    FLOAT,
    BOOL,
    SUBROUTINE,
}

pub struct ByteOps {}

impl ByteOps {
    // takes a sized struct and returns the in-memory byte representation
    // zero-copy, see: https://stackoverflow.com/questions/28127165/how-to-convert-struct-to-u8
    pub unsafe fn as_bytes<S: Sized>(s: &S) -> &[u8] {
        let byte_slice_repr =
            slice::from_raw_parts((s as *const S) as *const u8, mem::size_of::<S>());
        return byte_slice_repr;
    }

    // returns the typed representation of a slice of bytes
    // zero-copy, this just returns the typed reference, does not copy any data.
    pub unsafe fn as_type<T: Sized>(buf: &[u8]) -> Option<T> {
        if buf.len() == mem::size_of::<T>() {
            let typed_ref_repr: T = mem::transmute_copy(&buf[0]);
            return Some(typed_ref_repr);
        } else {
            return None;
        }
    }

    pub fn repr_boson_float(f: &f64) -> Option<Vec<u8>> {
        let mut bytes = [0u8; mem::size_of::<f64>()];
        if USE_BIG_ENDIAN_REPR {
            let result = bytes.as_mut().write_f64::<BigEndian>(*f);
            if result.is_err() {
                return None;
            }
        } else {
            let result = bytes.as_mut().write_f64::<LittleEndian>(*f);
            if result.is_err() {
                return None;
            }
        }

        return Some(bytes.to_vec());
    }

    pub fn repr_boson_int(i: &i64) -> Option<Vec<u8>> {
        let mut bytes = [0u8; mem::size_of::<f64>()];
        if USE_BIG_ENDIAN_REPR {
            let result = bytes.as_mut().write_i64::<BigEndian>(*i);
            if result.is_err() {
                return None;
            }
        } else {
            let result = bytes.as_mut().write_i64::<LittleEndian>(*i);
            if result.is_err() {
                return None;
            }
        }

        return Some(bytes.to_vec());
    }

    pub fn generate_magic() -> u64 {
        let result = unsafe { ByteOps::as_type::<u64>(MAGIC.as_bytes()) };

        match result {
            Some(u64_magic) => u64_magic,
            None => 0,
        }
    }
}

#[repr(C, packed)]
pub struct DataIndexItem {
    pub const_idx: i32,
    pub start: u64,
    pub end: u64,
    pub t_code: TypeCode,
}

#[repr(C, packed)]
pub struct Header {
    pub magic: u64,
    pub num_data: u64,
    pub num_sub: u64,
    pub data_end_idx: u64,
    pub sub_end_idx: u64,
}

#[repr(C, packed)]
pub struct SubroutineIndexItem {
    pub name_data_idx: u64,
    pub n_locals: u64,
    pub n_params: u64,
    pub code_idx: u64,
    pub const_idx: i32,
}

// organization of bytecode file:

/*
    | header = contains info about subroutine-index and data index |
    | subroutine-index = mapping to subroutine in bytes area |
    | data-index = contains mapping of data in bytes area|
    | bytes-area |
*/

pub struct BytecodeWriter {
    pub current_size: usize,
    pub data_items: Vec<DataIndexItem>,
    pub subroutine_items: Vec<SubroutineIndexItem>,
    pub header: Header,
    pub bin_pool: Vec<u8>,
}

impl BytecodeWriter {
    pub fn new() -> BytecodeWriter {
        BytecodeWriter {
            current_size: 0,
            data_items: vec![],
            subroutine_items: vec![],
            header: Header {
                magic: ByteOps::generate_magic(),
                num_data: 0,
                num_sub: 0,
                data_end_idx: 0,
                sub_end_idx: 0,
            },
            bin_pool: vec![],
        }
    }

    fn new_data_idx(&mut self, idx: i32, t: TypeCode, data: &[u8]) -> u64 {
        let bin_start = self.bin_pool.len();

        // extend the bin pool with new data:
        self.bin_pool.extend(data);

        let bin_end = self.bin_pool.len();
        let data_item = DataIndexItem {
            const_idx: idx,
            start: bin_start as u64,
            end: bin_end as u64,
            t_code: t,
        };

        self.current_size = bin_end;
        self.data_items.push(data_item);

        return self.data_items.len() as u64;
    }

    fn new_subroutine_idx(
        &mut self,
        const_idx: i32,
        name: String,
        n_p: usize,
        n_l: usize,
        code: &CompiledInstructions,
    ) -> u64 {
        // create a data-index for name:
        let name_data_idx = self.new_data_idx(const_idx, TypeCode::STR, &name.as_bytes());
        let code_idx = self.new_data_idx(const_idx, TypeCode::SUBROUTINE, &code);

        let subroutine = SubroutineIndexItem {
            name_data_idx,
            n_locals: n_l as u64,
            n_params: n_p as u64,
            code_idx,
            const_idx,
        };

        // push to subroutine pool:
        self.subroutine_items.push(subroutine);

        // ad

        return self.subroutine_items.len() as u64;
    }

    fn encode_to_binary(&mut self, bytecode: &CompiledBytecode) -> Result<Vec<u8>, String> {
        // prepare the main function subroutine pool:

        // main function:
        self.new_subroutine_idx(-1, "main".to_string(), 0, 0, &bytecode.instructions);

        let mut current_count = 0;
        // now compile the constant pool:
        for object in &bytecode.constant_pool.objects {
            match object.as_ref() {
                Object::Bool(b) => {
                    let b_val = if *b { vec![1u8] } else { vec![0u8] };
                    self.new_data_idx(current_count as i32, TypeCode::BOOL, &b_val);
                }

                Object::Str(st) => {
                    let b_val = st.as_bytes();
                    self.new_data_idx(current_count as i32, TypeCode::STR, &b_val);
                }

                Object::Char(ch) => {
                    let b_val = vec![*ch as u8];
                    self.new_data_idx(current_count as i32, TypeCode::CHAR, &b_val);
                }

                Object::Int(i) => {
                    let b_res = ByteOps::repr_boson_int(i);
                    if b_res.is_none() {
                        return Err(format!("Failed to serialize int {}", i));
                    }

                    self.new_data_idx(current_count as i32, TypeCode::INT, &b_res.unwrap());
                }

                Object::Float(f) => {
                    let b_res = ByteOps::repr_boson_float(f);
                    if b_res.is_none() {
                        return Err(format!("Failed to serialize int {}", f));
                    }

                    self.new_data_idx(current_count as i32, TypeCode::FLOAT, &b_res.unwrap());
                }

                Object::Subroutine(sub) => {
                    self.new_subroutine_idx(
                        current_count as i32,
                        sub.get_name().clone(),
                        sub.gen_n_parameters(),
                        sub.get_n_locals(),
                        &sub.as_ref().bytecode,
                    );
                }

                Object::Noval => {
                    self.new_data_idx(current_count as i32, TypeCode::NONE, &vec![0u8]);
                }

                _ => {
                    return Err(format!(
                        "Object {} cannot be serialized.",
                        object.get_type()
                    ));
                }
            }
            current_count += 1;
        }

        let mut data_idx_bin: Vec<u8> = vec![];
        let mut sub_idx_bin: Vec<u8> = vec![];

        // serialize data-index:
        for data_idx in &self.data_items {
            let sliced_repr = unsafe { ByteOps::as_bytes::<DataIndexItem>(data_idx) };
            data_idx_bin.extend(sliced_repr);
        }

        for sub_idx in &self.subroutine_items {
            let sliced_repr = unsafe { ByteOps::as_bytes::<SubroutineIndexItem>(sub_idx) };
            sub_idx_bin.extend(sliced_repr);
        }

        self.header.num_data = self.data_items.len() as u64;
        self.header.num_sub = self.subroutine_items.len() as u64;

        self.header.sub_end_idx = (mem::size_of::<Header>() + sub_idx_bin.len()) as u64;
        self.header.data_end_idx = self.header.sub_end_idx + (data_idx_bin.len() as u64);

        // serialize header:
        let header_slice = unsafe { ByteOps::as_bytes::<Header>(&self.header) };

        let mut ser_bytecode: Vec<u8> = vec![];
        ser_bytecode.extend(header_slice);
        ser_bytecode.extend(sub_idx_bin);
        ser_bytecode.extend(data_idx_bin);
        ser_bytecode.extend(&self.bin_pool);

        return Ok(ser_bytecode);
    }

    // returns the size of bytecode returned or an error string
    pub fn save_bytecode(
        &mut self,
        fname: String,
        bytecode: &CompiledBytecode,
    ) -> Result<usize, String> {
        let ser_result = self.encode_to_binary(bytecode);
        if ser_result.is_err() {
            return Err(ser_result.unwrap_err());
        }

        // save bytecode to fine:
        let content = ser_result.unwrap();
        let io_result = fs::write(&fname, &content);
        if io_result.is_err() {
            return Err(format!("Failed to write bytecode to file {}", fname));
        }

        // return the data
        return Ok(content.len());
    }
}

pub struct BytecodeLoader {
    pub name: String,
    pub bin: Vec<u8>,
    pub data_table: HashMap<i32, Vec<DataIndexItem>>,
    pub subroutine_table: HashMap<i32, Vec<SubroutineIndexItem>>,
}

impl BytecodeLoader {
    pub fn new(fname: String) -> BytecodeLoader {
        BytecodeLoader {
            name: fname,
            bin: vec![],
            data_table: HashMap::new(),
            subroutine_table: HashMap::new(),
        }
    }

    fn __verify_magic(&self, magic_slice: &[u8]) -> Result<(), String> {
        let stringified_magic = String::from_utf8_lossy(magic_slice);
        if stringified_magic.as_ref() != MAGIC {
            return Err(format!("{} is not a valid Boson magic", stringified_magic));
        }

        return Ok(());
    }

    fn __build_subroutine_map(&mut self, h: &Header) -> Result<(), String> {
        let sub_section = &self.bin[mem::size_of::<Header>()..(h.sub_end_idx as usize)];
        let item_size = mem::size_of::<SubroutineIndexItem>();

        for idx in 0..h.num_sub {
            let item_slice =
                &sub_section[(idx as usize * item_size)..((idx + 1) as usize * item_size)];
            let sub_item_res = unsafe { ByteOps::as_type::<SubroutineIndexItem>(&item_slice) };
            if sub_item_res.is_none() {
                return Err(format!(
                    "SubroutineIndexItem cannot be derived from {:?}",
                    item_slice
                ));
            }

            let sub_item: SubroutineIndexItem = sub_item_res.unwrap();
            self.subroutine_table
                .entry(sub_item.const_idx)
                .or_insert(vec![])
                .push(sub_item);
        }

        return Ok(());
    }

    fn __init(&mut self) -> Result<(), String> {
        let bin_read_res = fs::read(&self.name);
        if bin_read_res.is_err() {
            return Err(format!(
                "Error loading {}, file could not be read.",
                self.name
            ));
        }

        let bin = bin_read_res.unwrap();

        // verify magic:
        let v_res = self.__verify_magic(&bin[0..8]);
        if v_res.is_err() {
            return Err(v_res.unwrap_err());
        }

        self.bin = bin;

        // read header:
        let header_slice = &self.bin[0..mem::size_of::<Header>()];
        let header_res = unsafe { ByteOps::as_type::<Header>(header_slice) };

        if header_res.is_none() {
            return Err(format!("Invalid header {:?}", header_slice));
        }

        let header: Header = header_res.unwrap();
        // make some checks:
        let has_aligned_subs = (header.sub_end_idx as usize - mem::size_of::<Header>())
            / mem::size_of::<SubroutineIndexItem>()
            == header.num_sub as usize;

        let has_aligned_data = (header.data_end_idx as usize
            - (mem::size_of::<Header>() + header.sub_end_idx as usize))
            / mem::size_of::<DataIndexItem>()
            == header.num_data as usize;

        if !has_aligned_data || !has_aligned_subs {
            return Err(format!(
                "Improper bytecode file {}, data and subroutine sections are not aligned.",
                self.name
            ));
        }

        let sub_build_res = self.__build_subroutine_map(&header);
        if sub_build_res.is_err() {
            return Err(sub_build_res.unwrap_err());
        }

        return Ok(());
    }
}
