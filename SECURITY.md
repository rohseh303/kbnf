# Security model for user-supplied grammars

KBNF grammars are executable workload descriptions. A valid grammar can request
large parser structures, expensive regex DFAs, large suffix automata, or a grammar
whose EBNF simplification creates far more productions than its source suggests.
Treat an arbitrary grammar like an untrusted query, not like inert configuration.

## Protected construction path

`Config::hardened()` and `kbnf.Config.hardened()` install conservative defaults for:

- source bytes and lexical nesting, checked before parsing;
- parsed AST nodes/depth and interned nonterminal, terminal, regex, and substring counts;
- terminal/substring bytes, per-regex bytes, and total regex bytes;
- an estimated regex NFA size (sub-expression sizes multiplied by counted repetition
  bounds), per regex and in total, checked before any regex is compiled;
- a saturating EBNF simplification-expansion estimate, checked before simplification;
- actual simplified production and symbol counts;
- regex DFA memory through the existing regex compiler limit; and
- a cooperative wall-clock deadline checked between construction phases.

Failures are structured `GrammarLimitError` values in Rust and `ValueError`s with the
same phase/resource/observed/limit message in Python. `inspect_grammar` provides the
deterministic pre-compilation metrics without compiling regexes; `check_grammar` runs
the whole construction pipeline under a policy without a vocabulary, so admission can
happen in a disposable worker before the inference process builds an engine.

## Protected decode path

Construction limits do not bound what happens once tokens flow. An ambiguous grammar
can make each Earley set grow with the length of the output, and the allowed-token
cache stores a copy of the chart plus a vocabulary-sized bitset for every distinct
parser state it sees. `Config::decode_limits` (`DecodeLimits`, enabled by
`Config::hardened()`) therefore caps:

- Earley items in the newest set after each accepted byte;
- Earley items across the whole chart; and
- retained allowed-token cache entries (the cache is cleared when full).

A token whose acceptance would exceed a chart budget is excluded from the allowed set
during `compute_allowed_token_ids`; forcing it through `try_accept_new_token` or
`update_logits` returns `ResourceLimitExceeded` and leaves the engine state unchanged.
Tokens admitted by the eager regex cache bypass the mask-time check and are still
rejected at accept time, so a service must treat that error as a terminal, fail-closed
condition for the request. `Engine::earley_chart_size` and `Engine::cache_size` expose
the counters for monitoring.

The unlimited `Config::default()` behavior is retained to avoid silently breaking
existing trusted workloads. A network service should never use that default on a
grammar supplied by a client.

## Required service boundary

In-process limits reduce the common amplification paths but cannot forcibly stop a
single call inside the parser, regex compiler, allocator, or a future dependency.
For a hard availability boundary:

1. Compile a grammar in a disposable worker process, not the inference process.
2. Apply an operating-system memory limit and a wall-clock deadline to that worker.
3. Cache only successful compiled artifacts under a hash of grammar, KBNF version,
   tokenizer identity, and the complete limits policy.
4. Bound cache size and reject—not merely log—policy violations.
5. Keep the inference worker's own request/output/token limits independent of these
   grammar-construction limits.

## Non-goals

This layer enforces syntactic resource policy. It does not decide whether a field's
meaning is safe, truthful, authorized, or consistent with another field. Semantic
constraints belong in a separate validation phase after syntactically valid output
has been produced.

## Testing

The `tests/adversarial_grammars` corpus records representative attack shapes, while
`tests/grammar_limits.rs` verifies early rejection, structural error details,
backwards compatibility, the hardened happy path, and expansion accounting. The
`fuzz/grammar_construction` target continuously probes that same hardened path. The
repository checks in lockfiles for reproducible Rust and fuzz builds; verify the
Rust suite, Python and WASM features, and fuzz target before release.
