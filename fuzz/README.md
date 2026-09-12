# Grammar-construction fuzzer

Install `cargo-fuzz`, then run:

```shell
cargo fuzz run grammar_construction tests/adversarial_grammars
```

The target sends arbitrary UTF-8 grammars through the hardened construction path.
The corpus in `tests/adversarial_grammars` provides useful initial shapes. Run the
fuzzer itself inside a resource-limited process: the in-process compile deadline is
cooperative and cannot preempt a single native parser or regex-compiler call.
