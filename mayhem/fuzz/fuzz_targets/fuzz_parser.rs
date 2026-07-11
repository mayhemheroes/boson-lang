#![no_main]

use libfuzzer_sys::fuzz_target;

use boson::lexer::LexerAPI;
use boson::parser::Parser;

// Ported verbatim from the old fork's boson/fuzz/fuzz_targets/parser.rs: lex + parse attacker
// bytes into an AST (Parser::new_from_lexer(..).parse()).
fuzz_target!(|data: &[u8]| {
    let vec = data.to_vec();

    if vec.len() < 2 {
        return;
    }

    let lexer = LexerAPI::new_from_buffer(vec);
    let mut parser = Parser::new_from_lexer(lexer);

    let _ = parser.parse();
});
