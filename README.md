# kbnf

[![crates.io](https://img.shields.io/crates/v/kbnf)](https://crates.io/crates/kbnf)
[![docs.rs](https://docs.rs/kbnf/badge.svg)](https://docs.rs/kbnf)
[![PyPI](https://img.shields.io/pypi/v/kbnf.svg)](https://pypi.python.org/pypi/kbnf)
![CI](https://github.com/Dan-wanna-M/kbnf/actions/workflows/CI.yml/badge.svg)
![PyPI Downloads](https://static.pepy.tech/badge/kbnf)

This crate provides a constrained decoding engine which ensures that a language model's output adheres strictly to the format defined by KBNF (Koishi's BNF), an enhanced variant of EBNF. KBNF includes features that enhance usability, notably embeddable regular expressions.

If you are interested in the design and implementation behind this crate, you may want to check out [my blog](https://dan-wanna-m.github.io/blog/).

## Features

- Supports full context free grammar with worst case O(m\*n^3) time complexity, where `n` is the generated text length and `m` is the vocabulary size.
- Asymptotically fastest for subclasses of context free grammar.
  - Guarantees worst case O(m*n) time complexity for every LR(k) grammar(which includes almost all practical grammars)
  - Achieves O(n) time complexity with caching eventually given that `n` has a fixed upper bound, or the grammar is regular.
- Vocabulary-independent.
  - BPE, BBPE, you-name-it, all types of vocabulary are supported.
- Supports UTF-8 characters in grammar.
- Embeddable regular expressions.

## Safely accepting custom grammars

The original API remains backwards-compatible and unlimited. In a service that
accepts grammars from users, opt into the hardened profile so malformed or
resource-amplifying inputs are rejected before they monopolize an inference worker:

```rust
use kbnf::{Config, Engine};

let config = Config::hardened();
let engine = Engine::with_config(user_grammar, vocabulary, config)?;
```

The profile bounds two phases:

- **Construction** (`Config::grammar_limits`, a `GrammarLimits`): source and AST size,
  nesting, string tables, regex source bytes, an estimated regex NFA size (so nested
  counted repetitions such as `(a{1,100}){1,100}` are refused before the regex compiler
  runs), DFA memory, estimated EBNF expansion, simplified productions/symbols, and a
  cooperative compile deadline.
- **Decoding** (`Config::decode_limits`, a `DecodeLimits`): the number of Earley items in
  the newest set and across the whole chart after every accepted byte, the number of
  entries retained in the allowed-token cache, and the number of lazily built regex token
  caches (`RegexConfig::lazy_token_cache` keeps engine construction independent of
  vocabulary size). A token that would exceed a chart budget is
  masked out during `compute_allowed_token_ids` and rejected with
  `AcceptTokenError::ResourceLimitExceeded` if forced, leaving the engine state untouched.

Each rejection includes its phase, stable resource name, observed value, and configured
limit. Every limit is optional and can be tuned individually.

Python users get the same policy, a cheap preflight report, and a vocabulary-free full
check that a service can run in an isolated worker before building an engine:

```python
import kbnf

config = kbnf.Config.hardened()
complexity = kbnf.inspect_grammar(user_grammar)          # parse-only metrics
complexity = kbnf.check_grammar(user_grammar, config)    # full pipeline under the policy
engine = kbnf.Engine(user_grammar, vocabulary, config)
engine.earley_chart_size(), engine.cache_size()          # decode-time observability
```

`max_compile_millis` is checked between construction phases; it cannot preempt a
single regex compiler call. Multi-tenant servers should also compile in an isolated
worker process with an external timeout and memory limit. See [SECURITY.md](SECURITY.md)
for the threat model and deployment boundary.

## Documentation

[Documentation and examples](https://docs.rs/kbnf/).

## Add to your project

Simply add it to your `Cargo.toml` or run `cargo add kbnf` in your command line.

## Performance

One of the goals of this crate is for the constrained decoding engine to be "fast." This can be interpreted both theoretically and practically.

Theoretically, this crate is designed to provide the asymptotically fastest algorithms for *each subclass of context free grammar.* By implementing an Earley recognizer with Leo optimization, this crate has successfully achieve linear time complexity for every LR(k) grammar and quadratic time complexity for every unambiguous grammar. For general context free grammar, things are more ambiguous(pun intended): while subcubic algorithms exist(although with a large constant), all other general-purpose parsing algorithms(like Earley, GLR, GLL...) are indeed cubic, like ours.

Practically, this crate tries to make the engine be as efficient as possible for grammars used in practice. While many improvements, such as Earley sets compaction and lazy caching, have been made, this is inherently an ongoing process. If you find the engine is a bottleneck in your application, feel free to [open an issue](https://github.com/Dan-wanna-M/blog/issues/new).
