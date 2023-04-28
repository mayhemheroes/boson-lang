#![no_main]

use libfuzzer_sys::fuzz_target;

use boson::lexer::LexerAPI;
use boson::parser::Parser;

fuzz_target!(|data: &[u8]| {
    let vec = data.to_vec();

    if vec.len() < 2 {
        return;
    }

    let _ = LexerAPI::new_from_buffer(vec);
});