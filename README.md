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

The profile caps source and AST size, nesting, string tables, regex source and DFA
memory, estimated EBNF expansion, simplified productions/symbols, and cooperative
compile time. Each rejection includes its phase, stable resource name, observed
value, and configured limit. Every limit can be tuned through `Config::grammar_limits`.

Python users get the same policy and a cheap preflight report:

```python
import kbnf

config = kbnf.Config.hardened()
complexity = kbnf.inspect_grammar(user_grammar)
engine = kbnf.InternalEngine(user_grammar, vocabulary, config)
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
