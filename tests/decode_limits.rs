use kbnf::{
    config::Config,
    engine::Engine,
    engine_like::{AcceptTokenError, UpdateLogitsError},
    grammar::CreateGrammarError,
    limits::{DecodeLimits, GrammarLimits},
    EngineLike, Token, Vocabulary,
};

/// `a ::= a a | 'x'` is exponentially ambiguous: after `k` bytes the newest Earley set holds
/// one `a ::= a . a` item per possible origin, so set size grows linearly with output length.
/// The trailing `;` keeps the engine from finishing after the first byte.
const AMBIGUOUS: &str = "start ::= a ';'; a ::= a a | 'x';";

fn vocab(tokens: &[&str]) -> Vocabulary {
    let mut id_to_token = ahash::AHashMap::default();
    let mut id_to_string = ahash::AHashMap::default();
    for (id, token) in tokens.iter().enumerate() {
        id_to_token.insert(id as u32, Token(token.as_bytes().to_vec().into_boxed_slice()));
        id_to_string.insert(id as u32, token.to_string());
    }
    Vocabulary::new(id_to_token, id_to_string).unwrap()
}

fn engine_with(grammar: &str, tokens: &[&str], decode_limits: DecodeLimits) -> Engine {
    let mut config = Config::default();
    config.decode_limits = decode_limits;
    Engine::with_config(grammar, vocab(tokens), config).unwrap()
}

/// Feeds token 0 until the engine reports a limit; returns how many tokens were accepted.
fn accept_until_limit(engine: &mut Engine, max_tokens: usize) -> Option<usize> {
    for accepted in 0..max_tokens {
        match engine.try_accept_new_token(0) {
            Ok(_) => {}
            Err(AcceptTokenError::ResourceLimitExceeded) => return Some(accepted),
            Err(other) => panic!("unexpected error {other:?}"),
        }
    }
    None
}

#[test]
fn unbounded_engine_accepts_long_ambiguous_input() {
    let mut engine = engine_with(AMBIGUOUS, &["x", ";"], DecodeLimits::default());
    assert_eq!(accept_until_limit(&mut engine, 64), None);
    let (newest, total) = engine.earley_chart_size();
    assert!(newest > 32, "expected linear set growth, got {newest}");
    assert!(total >= newest);
}

#[test]
fn per_set_budget_rejects_token_and_leaves_state_untouched() {
    let limits = DecodeLimits {
        max_earley_items_per_set: Some(12),
        ..Default::default()
    };
    let mut engine = engine_with(AMBIGUOUS, &["x", ";"], limits);
    let accepted = accept_until_limit(&mut engine, 64).expect("budget should trigger");
    assert!(accepted > 0 && accepted < 64, "accepted {accepted}");
    let (newest, _) = engine.earley_chart_size();
    assert!(newest <= 12);

    // The failed accept must not have changed the parser state.
    let before = format!("{engine:?}");
    assert_eq!(
        engine.try_accept_new_token(0),
        Err(AcceptTokenError::ResourceLimitExceeded)
    );
    assert_eq!(before, format!("{engine:?}"));
    assert!(!engine.is_finished());
}

#[test]
fn per_set_budget_masks_tokens_that_would_exceed_it() {
    let limits = DecodeLimits {
        max_earley_items_per_set: Some(12),
        ..Default::default()
    };
    let mut engine = engine_with(AMBIGUOUS, &["x", ";"], limits);
    accept_until_limit(&mut engine, 64).expect("budget should trigger");
    engine.compute_allowed_token_ids();
    let allowed = engine.allowed_token_ids_from_last_computation();
    assert!(
        !allowed.contains(0),
        "a token whose acceptance would exceed the budget must be masked out"
    );
    assert!(
        allowed.contains(1),
        "the terminator keeps the newest set small and must stay allowed"
    );
    assert_eq!(engine.try_accept_new_token(1), Ok(kbnf::AcceptTokenResult::Finished));
}

#[test]
fn total_chart_budget_is_enforced() {
    let limits = DecodeLimits {
        max_total_earley_items: Some(60),
        ..Default::default()
    };
    let mut engine = engine_with(AMBIGUOUS, &["x", ";"], limits);
    accept_until_limit(&mut engine, 64).expect("total budget should trigger");
    let (_, total) = engine.earley_chart_size();
    assert!(total <= 60);
}

#[test]
fn update_logits_reports_structured_limit_error() {
    let limits = DecodeLimits {
        max_earley_items_per_set: Some(12),
        ..Default::default()
    };
    let mut engine = engine_with(AMBIGUOUS, &["x", ";"], limits);
    accept_until_limit(&mut engine, 64).expect("budget should trigger");
    let mut logits = vec![0.0f32; 2];
    assert_eq!(
        engine.update_logits(0, &mut logits),
        Err(UpdateLogitsError::ResourceLimitExceeded)
    );
}

#[test]
fn cache_entries_are_bounded() {
    let grammar = "start ::= 'a' 'b' 'c' 'd' 'e' 'f';";
    let tokens = ["a", "b", "c", "d", "e", "f"];

    let mut unbounded = engine_with(grammar, &tokens, DecodeLimits::default());
    for id in 0..6u32 {
        unbounded.compute_allowed_token_ids();
        unbounded.try_accept_new_token(id).unwrap();
    }
    assert_eq!(unbounded.cache_size(), 6);

    let mut bounded = engine_with(
        grammar,
        &tokens,
        DecodeLimits {
            max_cache_entries: Some(2),
            ..Default::default()
        },
    );
    for id in 0..6u32 {
        bounded.compute_allowed_token_ids();
        assert!(bounded.cache_size() <= 2);
        bounded.try_accept_new_token(id).unwrap();
    }

    let mut disabled = engine_with(
        grammar,
        &tokens,
        DecodeLimits {
            max_cache_entries: Some(0),
            ..Default::default()
        },
    );
    for id in 0..6u32 {
        disabled.compute_allowed_token_ids();
        assert_eq!(disabled.cache_size(), 0);
        disabled.try_accept_new_token(id).unwrap();
    }
}

#[test]
fn hardened_config_enables_decode_limits() {
    let config = Config::hardened();
    assert_eq!(config.decode_limits, DecodeLimits::hardened());
    assert!(config.decode_limits.max_earley_items_per_set.is_some());
    assert!(config.decode_limits.max_cache_entries.is_some());
    let engine = Engine::with_config("start ::= 'x';", vocab(&["x"]), config).unwrap();
    assert_eq!(engine.decode_limits(), DecodeLimits::hardened());
}

#[test]
fn check_grammar_reports_simplified_metrics_without_a_vocabulary() {
    let complexity =
        kbnf::utils::check_grammar("start ::= 'a' | 'b' | 'c';", Config::default().internal_config())
            .unwrap();
    assert_eq!(complexity.simplified_productions, 3);
    assert!(complexity.simplified_symbols >= 3);

    let mut config = Config::default();
    config.grammar_limits = GrammarLimits {
        max_simplified_productions: Some(2),
        ..Default::default()
    };
    match kbnf::utils::check_grammar("start ::= 'a' | 'b' | 'c';", config.internal_config()) {
        Err(CreateGrammarError::ResourceLimitError(error)) => {
            assert_eq!(error.resource, "simplified_productions");
            assert_eq!(error.observed, 3);
        }
        other => panic!("expected limit error, got {other:?}"),
    }
}
