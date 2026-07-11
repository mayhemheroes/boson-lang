#![no_main]

use libfuzzer_sys::fuzz_target;

use boson::lexer::LexerAPI;

// Ported verbatim from the old fork's boson/fuzz/fuzz_targets/lexer.rs: drive the boson
// lexer over attacker bytes (LexerAPI::new_from_buffer runs the tokenizer on construction).
fuzz_target!(|data: &[u8]| {
    let vec = data.to_vec();

    if vec.len() < 2 {
        return;
    }

    let _ = LexerAPI::new_from_buffer(vec);
});
