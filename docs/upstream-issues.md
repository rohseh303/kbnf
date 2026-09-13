# Upstream issues found while hardening kbnf

Both defects live in `kbnf-syntax` 0.5.3 (`src/parser.rs`, `remove_comment`) and were found by
fuzzing the grammar construction path. This fork works around them (see `SECURITY.md`);
the one-line fixes below belong upstream.

## 1. Unterminated `(*` comment hangs the parser forever

```rust
fn remove_comment(input: &str) -> Res<&str, Option<&str>> {
    let mut remove = delimited(
        complete::multispace0,
        opt(delimited(tag("(*"), take_until("*)"), tag("*)"))),
        complete::multispace0,
    );
    let (mut input, _) = remove(input)?;
    while input.len() >= 2 && &input[..2] == "(*" {   // never advances when "*)" is missing
        (input, _) = remove(input)?;
    }
    Ok((input, None))
}
```

When the closing `*)` is missing, `take_until("*)")` fails, `opt` yields `None` without consuming
anything, the loop condition is still true, and the loop never terminates. Any grammar that
starts with `(*` (two bytes) pins a CPU core indefinitely.

Repro: `kbnf_syntax::get_grammar("(*")` (or `kbnf.Engine("(*", vocab)` from Python).

Fix: fail when the optional comment parser makes no progress, e.g.

```rust
while input.starts_with("(*") {
    let (rest, _) = remove(input)?;
    if rest.len() == input.len() {
        return Err(nom::Err::Failure(VerboseError::from_error_kind(input, ErrorKind::TakeUntil)));
    }
    input = rest;
}
```

## 2. Byte-index slice panics on multi-byte input

The same loop uses `&input[..2] == "(*"`. If the first character is a multi-byte UTF-8
character (for example `𝏾` or U+FFFD), byte index 2 is not a char boundary and the slice
panics: `byte index 2 is not a char boundary; it is inside '\u{1d3fe}' (bytes 0..4)`.
Through PyO3 this surfaces as `pyo3_runtime.PanicException`, which is a `BaseException`
subclass that ordinary `except Exception` handlers do not catch.

Repro: `kbnf_syntax::get_grammar("\u{1d3fe}")`.

Fix: `input.starts_with("(*")` instead of slicing.

## Measured recursion limits (for the record)

The parser recurses once per alternative and once per parenthesis level. On an 8 MB stack it
overflows near 32,000 alternatives in one rule and near 16,000 nested groups (`inspect`-only,
no engine). This fork bounds both lexically before parsing (`GrammarLimits::hardened()` uses
4,096 alternatives per rule and a structural nesting depth of 128).
