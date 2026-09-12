#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(grammar) = std::str::from_utf8(data) else {
        return;
    };
    let mut config = kbnf::Config::hardened();
    config.grammar_limits.max_source_bytes = Some(64 * 1024);
    config.grammar_limits.max_compile_millis = Some(250);
    let _ = kbnf::utils::construct_kbnf_syntax_grammar(grammar, config.internal_config());
});
