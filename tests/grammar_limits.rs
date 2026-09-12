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
