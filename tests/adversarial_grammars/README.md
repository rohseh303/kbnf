# Adversarial grammar corpus

These fixtures represent resource-amplification shapes that an inference service
should reject under a suitably strict `GrammarLimits` policy. The automated tests
generate scaled variants so the checked-in corpus stays reviewable.

- `nested.kbnf`: parser/AST depth pressure.
- `optional_expansion.kbnf`: EBNF simplification fan-out.
- `regex_expansion.kbnf`: DFA construction pressure.
- `recursive.kbnf`: legal recursion that must remain supported.

The resource policy is deterministic for source, AST, string-table, and simplified
grammar counts. `max_compile_millis` is a cooperative deadline; run compilation in
an isolated worker process when a hard preemptive deadline is required.
