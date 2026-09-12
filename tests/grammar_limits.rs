use kbnf::{
    config::Config,
    engine::{CreateEngineError, Engine},
    grammar::CreateGrammarError,
    limits::{GrammarLimits, GrammarPhase},
    utils::construct_kbnf_syntax_grammar,
    Token, Vocabulary,
};

fn compile_with_limits(grammar: &str, limits: GrammarLimits) -> Result<(), CreateGrammarError> {
    let mut config = Config::default();
    config.grammar_limits = limits;
    config.compression_config.min_terminals = usize::MAX;
    construct_kbnf_syntax_grammar(grammar, config.internal_config()).map(|_| ())
}

fn assert_limit(
    result: Result<(), CreateGrammarError>,
    phase: GrammarPhase,
    resource: &'static str,
    observed: usize,
    limit: usize,
) {
    match result {
        Err(CreateGrammarError::ResourceLimitError(error)) => {
            assert_eq!(error.phase, phase);
            assert_eq!(error.resource, resource);
            assert_eq!(error.observed, observed);
            assert_eq!(error.limit, limit);
        }
        other => panic!("expected resource limit error, got {other:?}"),
    }
}

#[test]
fn rejects_source_before_parsing() {
    let limits = GrammarLimits {
        max_source_bytes: Some(8),
        ..Default::default()
    };
    assert_limit(
        compile_with_limits("not valid grammar", limits),
        GrammarPhase::Source,
        "source_bytes",
        17,
        8,
    );
}

#[test]
fn rejects_deeply_nested_ast() {
    let limits = GrammarLimits {
        max_nesting_depth: Some(3),
        ..Default::default()
    };
    let result = compile_with_limits("start ::= (((('a'))));", limits);
    match result {
        Err(CreateGrammarError::ResourceLimitError(error)) => {
            assert_eq!(error.phase, GrammarPhase::Source);
            assert_eq!(error.resource, "nesting_depth");
            assert!(error.observed > error.limit);
        }
        other => panic!("expected nesting limit error, got {other:?}"),
    }
}

#[test]
fn rejects_large_regex_before_compilation() {
    let limits = GrammarLimits {
        max_regex_bytes: Some(4),
        ..Default::default()
    };
    assert_limit(
        compile_with_limits("start ::= #'abcdef';", limits),
        GrammarPhase::Parsed,
        "max_regex_bytes",
        14,
        4,
    );
}

#[test]
fn rejects_excessive_simplified_productions() {
    let limits = GrammarLimits {
        max_simplified_productions: Some(10),
        ..Default::default()
    };
    let result = compile_with_limits(
        "start ::= 'a' | 'b' | 'c' | 'd' | 'e' | 'f' | 'g' | 'h' | 'i' | 'j' | 'k';",
        limits,
    );
    match result {
        Err(CreateGrammarError::ResourceLimitError(error)) => {
            assert_eq!(error.phase, GrammarPhase::Simplified);
            assert_eq!(error.resource, "simplified_productions");
            assert!(error.observed > error.limit);
        }
        other => panic!("expected simplification limit error, got {other:?}"),
    }
}

#[test]
fn rejects_estimated_expansion_before_simplification() {
    let limits = GrammarLimits {
        max_simplification_expansion: Some(16),
        ..Default::default()
    };
    let result = compile_with_limits("start ::= 'a'? 'b'? 'c'? 'd'? 'e'?;", limits);
    match result {
        Err(CreateGrammarError::ResourceLimitError(error)) => {
            assert_eq!(error.phase, GrammarPhase::Parsed);
            assert_eq!(error.resource, "simplification_expansion");
            assert_eq!(error.observed, 32);
            assert_eq!(error.limit, 16);
        }
        other => panic!("expected expansion limit error, got {other:?}"),
    }
}

#[test]
fn inspection_reports_complexity_without_compiling() {
    let complexity = kbnf::utils::inspect_grammar("start ::= 'a'? 'b'?;").unwrap();
    assert_eq!(complexity.source_bytes, 20);
    assert_eq!(complexity.simplification_expansion, 4);
    assert_eq!(complexity.simplified_productions, 0);
}

#[test]
fn compile_deadline_is_reported_structurally() {
    let limits = GrammarLimits {
        max_compile_millis: Some(0),
        ..Default::default()
    };
    assert_limit(
        compile_with_limits("start ::= 'a';", limits),
        GrammarPhase::Source,
        "compile_millis",
        0,
        0,
    );
}

#[test]
fn unlimited_default_preserves_existing_behavior() {
    compile_with_limits("start ::= 'a' | #'[0-9]+';", GrammarLimits::default()).unwrap();
}

#[test]
fn hardened_profile_accepts_normal_schema() {
    compile_with_limits(
        r#"start ::= '{' '"name"' ':' #'"[a-zA-Z ]+"' '}';"#,
        GrammarLimits::hardened(),
    )
    .unwrap();
}

#[test]
fn engine_creation_propagates_structured_limit_error() {
    let mut config = Config::hardened();
    config.grammar_limits.max_source_bytes = Some(8);
    let mut tokens = ahash::AHashMap::default();
    tokens.insert(0, Token(b"ok".to_vec().into_boxed_slice()));
    let mut token_strings = ahash::AHashMap::default();
    token_strings.insert(0, "ok".to_string());
    let vocabulary = Vocabulary::new(tokens, token_strings).unwrap();

    match Engine::with_config("start ::= 'ok';", vocabulary, config) {
        Err(CreateEngineError::GrammarError(CreateGrammarError::ResourceLimitError(error))) => {
            assert_eq!(error.phase, GrammarPhase::Source);
            assert_eq!(error.resource, "source_bytes");
        }
        other => panic!("expected engine resource limit error, got {other:?}"),
    }
}

#[test]
fn adversarial_corpus_exercises_expected_boundaries() {
    let recursive = include_str!("adversarial_grammars/recursive.kbnf");
    compile_with_limits(recursive, GrammarLimits::hardened()).unwrap();

    let nested_limits = GrammarLimits {
        max_nesting_depth: Some(4),
        ..Default::default()
    };
    assert!(matches!(
        compile_with_limits(
            include_str!("adversarial_grammars/nested.kbnf"),
            nested_limits
        ),
        Err(CreateGrammarError::ResourceLimitError(_))
    ));

    let expansion_limits = GrammarLimits {
        max_simplification_expansion: Some(32),
        ..Default::default()
    };
    assert!(matches!(
        compile_with_limits(
            include_str!("adversarial_grammars/optional_expansion.kbnf"),
            expansion_limits
        ),
        Err(CreateGrammarError::ResourceLimitError(_))
    ));

    let regex_limits = GrammarLimits {
        max_regex_bytes: Some(16),
        ..Default::default()
    };
    assert!(matches!(
        compile_with_limits(
            include_str!("adversarial_grammars/regex_expansion.kbnf"),
            regex_limits
        ),
        Err(CreateGrammarError::ResourceLimitError(_))
    ));

    let nested_repetition_limits = GrammarLimits {
        max_regex_size_estimate: Some(1_000),
        ..Default::default()
    };
    assert!(matches!(
        compile_with_limits(
            include_str!("adversarial_grammars/nested_repetition.kbnf"),
            nested_repetition_limits
        ),
        Err(CreateGrammarError::ResourceLimitError(_))
    ));
}

#[test]
fn lexical_depth_scanner_ignores_parentheses_in_regexes_and_comments() {
    let limits = GrammarLimits {
        max_nesting_depth: Some(1),
        ..Default::default()
    };
    compile_with_limits(r#"(* ((( ignored ))) *) start ::= #'(a|b)+';"#, limits).unwrap();
}

#[test]
fn nested_counted_repetition_is_rejected_by_size_estimate() {
    let limits = GrammarLimits {
        max_regex_size_estimate: Some(5_000),
        ..Default::default()
    };
    match compile_with_limits("start ::= #'(a{1,100}){1,100}';", limits) {
        Err(CreateGrammarError::ResourceLimitError(error)) => {
            assert_eq!(error.phase, GrammarPhase::Parsed);
            assert_eq!(error.resource, "regex_size_estimate");
            assert!(error.observed >= 10_000, "observed {}", error.observed);
            assert_eq!(error.limit, 5_000);
        }
        other => panic!("expected regex size limit error, got {other:?}"),
    }
}

#[test]
fn regex_size_estimate_scales_with_repetition_bounds() {
    let small = kbnf::limits::regex_size_estimate("[a-z]+");
    let counted = kbnf::limits::regex_size_estimate("[a-z]{1,64}");
    let nested = kbnf::limits::regex_size_estimate("([a-z]{1,64}){1,64}");
    assert!(small < counted && counted < nested, "{small} {counted} {nested}");
    assert!(nested >= 64 * 64);
    assert_eq!(kbnf::limits::regex_size_estimate("(unbalanced"), 0);

    let complexity = kbnf::utils::inspect_grammar("start ::= #'[a-z]{1,64}' #'x';").unwrap();
    assert!(complexity.regex_size_estimate >= counted);
    assert!(complexity.regex_size_estimate < counted + 8);
    assert!(complexity.total_regex_size_estimate > complexity.regex_size_estimate);
    compile_with_limits("start ::= #'[a-z]{1,64}';", GrammarLimits::hardened()).unwrap();
}

/// Dependency-free randomized robustness tests (a fixed-seed xorshift keeps them deterministic).
mod randomized {
    use super::*;

    struct XorShift(u64);
    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
            items[(self.next() % items.len() as u64) as usize]
        }
    }

    const FRAGMENTS: &[&str] = &[
        "start", "::=", ";", "|", "(", ")", "?", "*", "+", "'a'", "'b'", "\"c\"", "#'[a-z]+'",
        "#'(a{1,50}){1,50}'", "#e'x'", "#substrs'abc'", "item", "\n", " ", "(*", "*)", "\\", "'",
        "start ::= item;", "item ::= 'x' | item ',' item;", "#'.{0,4096}'", "½©", "𝏾", "'񆃫'",
        "#'[^\\u0000-\\u001f]'", "{", "}", "[", "]", "::", "=",
    ];

    /// Hardened construction must return an error, never panic, whatever the input looks like.
    #[test]
    fn hardened_construction_never_panics_on_fragment_soup() {
        let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
        for _ in 0..2_000 {
            let len = 1 + (rng.next() % 40) as usize;
            let grammar: Vec<&str> = (0..len).map(|_| rng.pick(FRAGMENTS)).collect();
            let grammar = grammar.join(" ");
            let mut config = Config::hardened();
            config.grammar_limits.max_source_bytes = Some(16 * 1024);
            let _ = kbnf::utils::inspect_grammar(&grammar);
            let _ = construct_kbnf_syntax_grammar(&grammar, config.internal_config());
        }
    }

    /// Arbitrary Unicode text, including multi-byte characters at every position.
    #[test]
    fn hardened_construction_never_panics_on_unicode_noise() {
        let alphabet: Vec<char> = "ab'\"#:=;|()?*+ \n\\½©𝏾񆃫[]{}^$.-".chars().collect();
        let mut rng = XorShift(0xD1B5_4A32_D192_ED03);
        for _ in 0..2_000 {
            let len = (rng.next() % 64) as usize;
            let grammar: String = (0..len)
                .map(|_| alphabet[(rng.next() % alphabet.len() as u64) as usize])
                .collect();
            let _ = kbnf::utils::inspect_grammar(&grammar);
            let _ = construct_kbnf_syntax_grammar(&grammar, Config::hardened().internal_config());
        }
    }
}

#[test]
fn parser_panics_become_structured_errors() {
    // kbnf-syntax 0.5.3 slices `&input[..2]` while skipping comments, which panics when the
    // grammar starts with a multi-byte character. The fork must report that, not unwind.
    for grammar in ["\u{1d3fe}.=\\*", "\u{460eb}\u{3bc0a}", "'\u{460eb}' } item"] {
        match kbnf::utils::inspect_grammar(grammar) {
            Err(CreateGrammarError::InternalPanic { phase, message }) => {
                assert_eq!(phase, GrammarPhase::Parsed);
                assert!(message.contains("char boundary"), "{message}");
            }
            Err(CreateGrammarError::ParsingError(_)) => {}
            other => panic!("expected an error for {grammar:?}, got {other:?}"),
        }
        assert!(compile_with_limits(grammar, GrammarLimits::hardened()).is_err());
    }
}

#[test]
fn unterminated_comments_are_rejected_instead_of_hanging() {
    // kbnf-syntax 0.5.3 loops forever on an unclosed `(*`; the fork must refuse it quickly.
    let started = std::time::Instant::now();
    for grammar in ["(*", "(* never closed", "start ::= 'a'; (* trailing", "(*)", "(**"] {
        match kbnf::utils::inspect_grammar(grammar) {
            Err(CreateGrammarError::ParsingError(error)) => {
                assert!(error.to_string().contains("unterminated comment"), "{error}");
            }
            other => panic!("expected parsing error for {grammar:?}, got {other:?}"),
        }
        assert!(compile_with_limits(grammar, GrammarLimits::hardened()).is_err());
    }
    assert!(started.elapsed() < std::time::Duration::from_secs(1));

    kbnf::utils::inspect_grammar("(* ok *) start ::= 'a'; (* also ok *)").unwrap();
    kbnf::utils::inspect_grammar("start ::= '(*' | \"(*\" | #'\\\\(\\\\*';").unwrap();
    assert_eq!(kbnf::limits::find_unterminated_comment("'(*' (* x *)"), None);
    assert_eq!(kbnf::limits::find_unterminated_comment("a (* b"), Some(2));
}

#[test]
fn long_alternation_chains_are_bounded_lexically_and_structurally() {
    let chain = format!(
        "start ::= {};",
        (0..5_000).map(|i| format!("'a{i}'")).collect::<Vec<_>>().join(" | ")
    );
    let started = std::time::Instant::now();
    assert_limit(
        compile_with_limits(&chain, GrammarLimits::hardened()),
        GrammarPhase::Source,
        "alternatives_per_rule",
        4_097,
        4_096,
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(1));

    // Alternation chains are not nesting: a 300-way enum has a shallow AST.
    let enum_rule = format!(
        "start ::= {};",
        (0..300).map(|i| format!("'e{i}'")).collect::<Vec<_>>().join(" | ")
    );
    let complexity = kbnf::utils::inspect_grammar(&enum_rule).unwrap();
    assert_eq!(complexity.max_alternatives, 300);
    assert!(complexity.nesting_depth <= 3, "depth {}", complexity.nesting_depth);
    compile_with_limits(&enum_rule, GrammarLimits::hardened()).unwrap();

    let limits = GrammarLimits {
        max_alternatives_per_rule: Some(100),
        ..Default::default()
    };
    let inside_quotes = "start ::= '|||||' | #'a|b|c';";
    compile_with_limits(inside_quotes, limits).unwrap();
}

#[test]
fn nullable_nonterminals_are_counted_in_the_expansion_estimate() {
    let grammar = format!("start ::= {}; a ::= 'x'?;", vec!["a"; 20].join(" "));
    let complexity = kbnf::utils::inspect_grammar(&grammar).unwrap();
    assert!(complexity.simplification_expansion >= 1 << 20, "{}", complexity.simplification_expansion);
    let limits = GrammarLimits {
        max_simplification_expansion: Some(10_000),
        ..Default::default()
    };
    match compile_with_limits(&grammar, limits) {
        Err(CreateGrammarError::ResourceLimitError(error)) => {
            assert_eq!(error.resource, "simplification_expansion");
            assert_eq!(error.phase, GrammarPhase::Parsed);
        }
        other => panic!("expected expansion limit error, got {other:?}"),
    }
    // Indirect nullability through an empty terminal and a nullable chain.
    let indirect = "start ::= b b b b b b b b b b b b; b ::= c; c ::= '' | 'y';";
    assert!(kbnf::utils::inspect_grammar(indirect).unwrap().simplification_expansion >= 1 << 12);
    // A grammar with no nullable symbols keeps a linear estimate.
    let linear = format!("start ::= {}; a ::= 'x';", vec!["a"; 20].join(" "));
    assert!(kbnf::utils::inspect_grammar(&linear).unwrap().simplification_expansion < 64);
}

#[test]
fn nested_counted_loops_are_penalised_but_flat_repetition_is_not() {
    // KBNF unescapes the string once, so each regex backslash is written twice.
    let json_string = r#"start ::= #'"([^\\\\"]|\\\\["\\\\/bfnrt]|\\\\u[0-9A-Fa-f]{4}){0,1024}"';"#;
    compile_with_limits(json_string, GrammarLimits::hardened()).unwrap();
    compile_with_limits("start ::= #'.{0,4096}';", GrammarLimits::hardened()).unwrap();
    compile_with_limits("start ::= #'([0-9]{3}-){200}';", GrammarLimits::hardened()).unwrap();
    for hostile in ["start ::= #'(a{1,100}){1,100}';", "start ::= #'([a-z]{1,64}){1,64}';"] {
        match compile_with_limits(hostile, GrammarLimits::hardened()) {
            Err(CreateGrammarError::ResourceLimitError(error)) => {
                assert_eq!(error.resource, "regex_size_estimate", "{hostile}");
            }
            other => panic!("expected regex size rejection for {hostile}, got {other:?}"),
        }
    }
}
