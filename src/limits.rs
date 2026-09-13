//! Resource accounting and limits for untrusted grammars.

use kbnf_syntax::{
    grammar::Grammar as ParsedGrammar,
    node::{NodeWithID, RegexExtKind},
    SymbolKind,
};
#[cfg(feature = "python")]
use pyo3::pyclass;
use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

/// Resource limits applied while parsing, validating, and simplifying a grammar.
///
/// Every field is optional. `None` preserves the original unlimited behavior.
/// Use [`GrammarLimits::hardened`] for conservative multi-tenant defaults.
#[cfg_attr(feature = "python", pyclass)]
#[cfg_attr(feature = "python", pyo3(get_all, set_all))]
#[cfg_attr(feature = "wasm", wasm_bindgen(inspectable))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(default)]
pub struct GrammarLimits {
    /// Maximum UTF-8 source size.
    pub max_source_bytes: Option<usize>,
    /// Maximum number of nodes in the parsed grammar AST.
    pub max_ast_nodes: Option<usize>,
    /// Maximum nesting depth in the parsed grammar AST.
    ///
    /// Chains of the same operator (`a | b | c`, `a b c`) count as one level; this bounds
    /// structural nesting such as parenthesised groups.
    pub max_nesting_depth: Option<usize>,
    /// Maximum alternatives (`|`) in any one rule, checked lexically before parsing and
    /// again on the AST.
    ///
    /// The upstream parser recurses once per alternative and overflows its stack at roughly
    /// 32,000 alternatives, so this must stay well below that.
    pub max_alternatives_per_rule: Option<usize>,
    /// Maximum number of unique nonterminals.
    pub max_nonterminals: Option<usize>,
    /// Maximum number of unique terminal strings.
    pub max_terminals: Option<usize>,
    /// Maximum number of unique regular expressions.
    pub max_regexes: Option<usize>,
    /// Maximum number of unique substring expressions.
    pub max_substrings: Option<usize>,
    /// Maximum combined bytes in terminal and substring literals.
    pub max_literal_bytes: Option<usize>,
    /// Maximum bytes in any one regular expression.
    pub max_regex_bytes: Option<usize>,
    /// Maximum combined bytes in all regular expressions.
    pub max_total_regex_bytes: Option<usize>,
    /// Maximum estimated NFA size of any one regular expression.
    ///
    /// The estimate multiplies sub-expression sizes by counted repetition bounds, so nested
    /// repetitions such as `(a{1,100}){1,100}` are rejected before the regex compiler runs.
    pub max_regex_size_estimate: Option<usize>,
    /// Maximum combined estimated NFA size across all regular expressions.
    pub max_total_regex_size_estimate: Option<usize>,
    /// Maximum static estimate of EBNF expansion work before simplification.
    pub max_simplification_expansion: Option<usize>,
    /// Maximum productions after grammar simplification.
    pub max_simplified_productions: Option<usize>,
    /// Maximum symbols across all simplified productions.
    pub max_simplified_symbols: Option<usize>,
    /// Maximum symbols in any one simplified production.
    pub max_symbols_per_production: Option<usize>,
    /// Cooperative wall-clock deadline for grammar construction, in milliseconds.
    ///
    /// The deadline is checked between construction phases. Use process isolation
    /// when a hard, preemptive deadline is required.
    pub max_compile_millis: Option<usize>,
}

impl GrammarLimits {
    /// Conservative limits intended for user-supplied grammars in an inference service.
    pub fn hardened() -> Self {
        Self {
            max_source_bytes: Some(1_048_576),
            max_ast_nodes: Some(100_000),
            max_nesting_depth: Some(128),
            max_alternatives_per_rule: Some(4_096),
            max_nonterminals: Some(4_096),
            max_terminals: Some(16_384),
            max_regexes: Some(1_024),
            max_substrings: Some(1_024),
            max_literal_bytes: Some(4_194_304),
            max_regex_bytes: Some(65_536),
            max_total_regex_bytes: Some(1_048_576),
            max_regex_size_estimate: Some(20_000),
            max_total_regex_size_estimate: Some(200_000),
            max_simplification_expansion: Some(1_000_000),
            max_simplified_productions: Some(100_000),
            max_simplified_symbols: Some(1_000_000),
            max_symbols_per_production: Some(16_384),
            max_compile_millis: Some(5_000),
        }
    }
}

/// Resource limits applied while an engine consumes tokens.
///
/// Every field is optional. `None` preserves the original unbounded behavior.
/// Use [`DecodeLimits::hardened`] for conservative multi-tenant defaults.
#[cfg_attr(feature = "python", pyclass)]
#[cfg_attr(feature = "python", pyo3(get_all, set_all))]
#[cfg_attr(feature = "wasm", wasm_bindgen(inspectable))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(default)]
pub struct DecodeLimits {
    /// Maximum Earley items allowed in the newest Earley set after a byte is accepted.
    ///
    /// Ambiguous grammars can make a single Earley set grow with the length of the
    /// output. A token whose acceptance would exceed this bound is rejected with
    /// [`AcceptTokenError::ResourceLimitExceeded`](crate::engine_like::AcceptTokenError)
    /// and the engine state is left unchanged.
    pub max_earley_items_per_set: Option<usize>,
    /// Maximum Earley items allowed across the whole chart after a byte is accepted.
    pub max_total_earley_items: Option<usize>,
    /// Maximum entries retained in the allowed-token cache of one engine.
    ///
    /// Every entry stores a copy of the Earley chart plus a vocabulary-sized bitset, so an
    /// unbounded cache grows linearly with the number of distinct parser states visited.
    /// The cache is cleared when it is full; `Some(0)` disables insertion entirely.
    pub max_cache_entries: Option<usize>,
    /// Maximum lazily built regex token caches retained per engine
    /// (see `RegexConfig::lazy_token_cache`).
    ///
    /// Each entry stores two vocabulary-sized bitsets, so with a 150k-token vocabulary an
    /// entry is about 38 KB. States beyond the cap fall back to the uncached path.
    pub max_regex_cache_states: Option<usize>,
}

impl DecodeLimits {
    /// Conservative limits intended for engines that decode user-supplied grammars.
    pub fn hardened() -> Self {
        Self {
            max_earley_items_per_set: Some(65_536),
            max_total_earley_items: Some(1_048_576),
            max_cache_entries: Some(1_024),
            max_regex_cache_states: Some(512),
        }
    }

    /// Returns `true` when the chart sizes are within the configured limits.
    #[inline]
    pub(crate) fn chart_within_limits(&self, newest_set_items: usize, total_items: usize) -> bool {
        self.max_earley_items_per_set
            .is_none_or(|limit| newest_set_items <= limit)
            && self
                .max_total_earley_items
                .is_none_or(|limit| total_items <= limit)
    }
}

/// Deterministic complexity measurements for a grammar.
#[cfg_attr(feature = "python", pyclass)]
#[cfg_attr(feature = "python", pyo3(get_all))]
#[cfg_attr(feature = "wasm", wasm_bindgen(inspectable))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub struct GrammarComplexity {
    /// UTF-8 source size.
    pub source_bytes: usize,
    /// Number of parsed AST nodes.
    pub ast_nodes: usize,
    /// Maximum parsed AST nesting depth (operator chains count as one level).
    pub nesting_depth: usize,
    /// Alternatives in the rule with the most alternatives.
    pub max_alternatives: usize,
    /// Number of unique nonterminals.
    pub nonterminals: usize,
    /// Number of unique terminals.
    pub terminals: usize,
    /// Number of unique regular expressions.
    pub regexes: usize,
    /// Number of unique substring expressions.
    pub substrings: usize,
    /// Combined bytes in terminal and substring literals.
    pub literal_bytes: usize,
    /// Bytes in the largest regular expression.
    pub max_regex_bytes: usize,
    /// Combined bytes in regular expressions.
    pub total_regex_bytes: usize,
    /// Largest estimated NFA size among the regular expressions.
    pub regex_size_estimate: usize,
    /// Combined estimated NFA size across all regular expressions.
    pub total_regex_size_estimate: usize,
    /// Saturating static estimate of EBNF expansion work.
    pub simplification_expansion: usize,
    /// Productions after simplification, if simplification has run.
    pub simplified_productions: usize,
    /// Symbols after simplification, if simplification has run.
    pub simplified_symbols: usize,
    /// Largest production after simplification, if simplification has run.
    pub max_symbols_per_production: usize,
}

impl GrammarComplexity {
    pub(crate) fn from_parsed(source: &str, grammar: &ParsedGrammar) -> Self {
        let mut ast_nodes = 0usize;
        let mut nesting_depth = 0usize;
        let mut max_alternatives = 0usize;
        let mut simplification_expansion = 0usize;
        let nullable = nullable_nonterminals(grammar);
        for expression in &grammar.expressions {
            let mut alternations = 0usize;
            let mut stack = vec![(&expression.rhs, 1usize)];
            while let Some((node, depth)) = stack.pop() {
                ast_nodes = ast_nodes.saturating_add(1);
                nesting_depth = nesting_depth.max(depth);
                match node {
                    NodeWithID::Multiple(nodes) => {
                        stack.extend(nodes.iter().map(|node| (node, depth.saturating_add(1))));
                    }
                    NodeWithID::RegexExt(node, _) | NodeWithID::Group(node) => {
                        stack.push((node, depth.saturating_add(1)));
                    }
                    NodeWithID::Symbol(lhs, kind, rhs) => {
                        if matches!(kind, SymbolKind::Alternation) {
                            alternations = alternations.saturating_add(1);
                        }
                        // A chain of the same operator is one syntactic level.
                        for child in [lhs.as_ref(), rhs.as_ref()] {
                            let same_operator = matches!(
                                (child, kind),
                                (NodeWithID::Symbol(_, SymbolKind::Alternation, _), SymbolKind::Alternation)
                                    | (NodeWithID::Symbol(_, SymbolKind::Concatenation, _), SymbolKind::Concatenation)
                            );
                            let child_depth = if same_operator { depth } else { depth.saturating_add(1) };
                            stack.push((child, child_depth));
                        }
                    }
                    NodeWithID::Terminal(_)
                    | NodeWithID::RegexString(_)
                    | NodeWithID::Nonterminal(_)
                    | NodeWithID::EarlyEndRegexString(_)
                    | NodeWithID::Substrings(_)
                    | NodeWithID::RegexComplement(_)
                    | NodeWithID::Unknown => {}
                }
            }
            max_alternatives = max_alternatives.max(alternations.saturating_add(1));
            simplification_expansion = simplification_expansion
                .saturating_add(Self::expansion_estimate(&expression.rhs, &nullable));
        }

        let literal_bytes = grammar
            .interned_strings
            .terminals
            .iter()
            .map(|(_, value)| value.len())
            .chain(
                grammar
                    .interned_strings
                    .sub_strings
                    .iter()
                    .map(|(_, value)| value.len()),
            )
            .fold(0usize, usize::saturating_add);
        let (total_regex_bytes, max_regex_bytes) = grammar
            .interned_strings
            .regex_strings
            .iter()
            .map(|(_, value)| value.len())
            .fold((0usize, 0usize), |(total, max), len| {
                (total.saturating_add(len), max.max(len))
            });
        let (total_regex_size_estimate, regex_size_estimate) = grammar
            .interned_strings
            .regex_strings
            .iter()
            .map(|(_, value)| regex_size_estimate(value))
            .fold((0usize, 0usize), |(total, max), size| {
                (total.saturating_add(size), max.max(size))
            });

        Self {
            source_bytes: source.len(),
            ast_nodes,
            nesting_depth,
            max_alternatives,
            nonterminals: grammar.interned_strings.nonterminals.len(),
            terminals: grammar.interned_strings.terminals.len(),
            regexes: grammar.interned_strings.regex_strings.len(),
            substrings: grammar.interned_strings.sub_strings.len(),
            literal_bytes,
            max_regex_bytes,
            total_regex_bytes,
            regex_size_estimate,
            total_regex_size_estimate,
            simplification_expansion,
            ..Self::default()
        }
    }

    fn expansion_estimate<S: std::hash::Hash + Eq>(
        root: &NodeWithID,
        nullable: &ahash::AHashSet<S>,
    ) -> usize
    where
        S: std::borrow::Borrow<S>,
        for<'a> &'a S: From<&'a S>,
        NodeWithID: NullableLookup<S>,
    {
        enum Visit<'a> {
            Enter(&'a NodeWithID),
            Exit(&'a NodeWithID),
        }

        let mut visits = vec![Visit::Enter(root)];
        let mut values = Vec::new();
        while let Some(visit) = visits.pop() {
            match visit {
                Visit::Enter(node) => {
                    visits.push(Visit::Exit(node));
                    match node {
                        NodeWithID::Multiple(nodes) => {
                            visits.extend(nodes.iter().rev().map(Visit::Enter));
                        }
                        NodeWithID::RegexExt(node, _) | NodeWithID::Group(node) => {
                            visits.push(Visit::Enter(node));
                        }
                        NodeWithID::Symbol(lhs, _, rhs) => {
                            visits.push(Visit::Enter(rhs));
                            visits.push(Visit::Enter(lhs));
                        }
                        _ => {}
                    }
                }
                Visit::Exit(node) => match node {
                    NodeWithID::Multiple(nodes) => {
                        let start = values.len().saturating_sub(nodes.len());
                        let value = values[start..]
                            .iter()
                            .copied()
                            .fold(1usize, usize::saturating_mul);
                        values.truncate(start);
                        values.push(value);
                    }
                    NodeWithID::RegexExt(_, kind) => {
                        let child = values.pop().unwrap_or(1);
                        values.push(match kind {
                            RegexExtKind::Optional | RegexExtKind::Repeat0 => {
                                child.saturating_add(1)
                            }
                            RegexExtKind::Repeat1 => child,
                        });
                    }
                    NodeWithID::Group(_) => {}
                    NodeWithID::Symbol(_, kind, _) => {
                        let rhs = values.pop().unwrap_or(1);
                        let lhs = values.pop().unwrap_or(1);
                        values.push(match kind {
                            SymbolKind::Concatenation => lhs.saturating_mul(rhs),
                            SymbolKind::Alternation => lhs.saturating_add(rhs),
                        });
                    }
                    // Nullable nonterminals are eliminated by duplicating every production
                    // that references them, so each reference doubles the expansion.
                    node @ NodeWithID::Nonterminal(_) => {
                        values.push(if node.is_nullable_reference(nullable) { 2 } else { 1 })
                    }
                    _ => values.push(1),
                },
            }
        }
        values.pop().unwrap_or(0)
    }

    pub(crate) fn add_simplified(
        mut self,
        grammar: &kbnf_syntax::simplified_grammar::SimplifiedGrammar,
    ) -> Self {
        for rhs in &grammar.expressions {
            self.simplified_productions = self
                .simplified_productions
                .saturating_add(rhs.alternations.len());
            for production in &rhs.alternations {
                let symbols = production.concatenations.len();
                self.simplified_symbols = self.simplified_symbols.saturating_add(symbols);
                self.max_symbols_per_production = self.max_symbols_per_production.max(symbols);
            }
        }
        self
    }
}

/// Returns the byte offset of a `(*` comment opener that is never closed by `*)`.
///
/// `kbnf-syntax` 0.5.3 skips comments with `opt(delimited(tag("(*"), take_until("*)"), ..))`
/// inside a `while input.starts_with("(*")` loop; when the closing `*)` is missing the
/// optional parser makes no progress and the loop never terminates. Quoted terminals and
/// regexes are skipped so `'(*'` stays a legal literal.
pub fn find_unterminated_comment(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut quote = None;
    let mut escaped = false;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if byte == b'\'' || byte == b'"' {
            quote = Some(byte);
            index += 1;
            continue;
        }
        if byte == b'(' && bytes.get(index + 1) == Some(&b'*') {
            match input[index + 2..].find("*)") {
                Some(close) => index += 2 + close + 2,
                None => return Some(index),
            }
            continue;
        }
        index += 1;
    }
    None
}

/// Estimates the Thompson NFA size of a regular expression from its parsed HIR.
///
/// Sub-expression sizes are multiplied by counted repetition bounds, so nested counted
/// repetitions grow multiplicatively, mirroring how the NFA compiler unrolls them. Patterns
/// the parser rejects contribute `0`; grammar validation reports their syntax error later.
pub fn regex_size_estimate(pattern: &str) -> usize {
    match regex_syntax::ParserBuilder::new().build().parse(pattern) {
        Ok(hir) => hir_size(&hir).0,
        Err(_) => 0,
    }
}

/// Counted repetitions with at least this bound are treated as loops whose nesting
/// multiplies determinization cost (`(a{1,100}){1,100}` takes seconds; `\u[0-9a-f]{4}`
/// inside a loop does not).
const NESTED_LOOP_THRESHOLD: usize = 8;

/// Returns `(estimated size, largest counted-repetition bound inside)`.
fn hir_size(hir: &regex_syntax::hir::Hir) -> (usize, usize) {
    use regex_syntax::hir::HirKind;
    match hir.kind() {
        HirKind::Empty => (1, 0),
        HirKind::Literal(literal) => (literal.0.len().max(1), 0),
        HirKind::Class(_) | HirKind::Look(_) => (1, 0),
        HirKind::Capture(capture) => hir_size(&capture.sub),
        HirKind::Repetition(repetition) => {
            let bound = match repetition.max {
                Some(max) => max as usize,
                None => (repetition.min as usize).saturating_add(1),
            }
            .max(1);
            let (sub_size, inner_loop) = hir_size(&repetition.sub);
            let penalty = if inner_loop >= NESTED_LOOP_THRESHOLD && bound >= NESTED_LOOP_THRESHOLD {
                inner_loop
            } else {
                1
            };
            (
                sub_size.saturating_mul(bound).saturating_mul(penalty).saturating_add(1),
                bound.max(inner_loop),
            )
        }
        HirKind::Concat(parts) | HirKind::Alternation(parts) => parts.iter().map(hir_size).fold(
            (1usize, 0usize),
            |(size, largest), (part_size, part_loop)| (size.saturating_add(part_size), largest.max(part_loop)),
        ),
    }
}

/// Construction phase in which a grammar exceeded its policy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GrammarPhase {
    /// Before parsing.
    Source,
    /// After parsing and before regex compilation.
    Parsed,
    /// After semantic validation and regex compilation.
    Validated,
    /// After EBNF operators have been simplified.
    Simplified,
    /// After the vocabulary-dependent engine has been built.
    Engine,
}

impl std::fmt::Display for GrammarPhase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}",
            match self {
                Self::Source => "source",
                Self::Parsed => "parsed",
                Self::Validated => "validated",
                Self::Simplified => "simplified",
                Self::Engine => "engine",
            }
        )
    }
}

/// A resource policy violation with machine-readable observed and allowed values.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "grammar resource limit exceeded during {phase}: {resource} observed {observed}, limit {limit}"
)]
pub struct GrammarLimitError {
    /// Construction phase in which the limit was checked.
    pub phase: GrammarPhase,
    /// Stable resource name suitable for logs and metrics.
    pub resource: &'static str,
    /// Measured value.
    pub observed: usize,
    /// Configured maximum.
    pub limit: usize,
}

impl GrammarLimits {
    fn check_one(
        phase: GrammarPhase,
        resource: &'static str,
        observed: usize,
        limit: Option<usize>,
    ) -> Result<(), GrammarLimitError> {
        if let Some(limit) = limit {
            if observed > limit {
                return Err(GrammarLimitError {
                    phase,
                    resource,
                    observed,
                    limit,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn check_source(&self, source_bytes: usize) -> Result<(), GrammarLimitError> {
        Self::check_one(
            GrammarPhase::Source,
            "source_bytes",
            source_bytes,
            self.max_source_bytes,
        )
    }

    pub(crate) fn check_parsed(
        &self,
        complexity: &GrammarComplexity,
    ) -> Result<(), GrammarLimitError> {
        let phase = GrammarPhase::Parsed;
        Self::check_one(phase, "ast_nodes", complexity.ast_nodes, self.max_ast_nodes)?;
        Self::check_one(
            phase,
            "nesting_depth",
            complexity.nesting_depth,
            self.max_nesting_depth,
        )?;
        Self::check_one(
            phase,
            "alternatives_per_rule",
            complexity.max_alternatives,
            self.max_alternatives_per_rule,
        )?;
        Self::check_one(
            phase,
            "nonterminals",
            complexity.nonterminals,
            self.max_nonterminals,
        )?;
        Self::check_one(phase, "terminals", complexity.terminals, self.max_terminals)?;
        Self::check_one(phase, "regexes", complexity.regexes, self.max_regexes)?;
        Self::check_one(
            phase,
            "substrings",
            complexity.substrings,
            self.max_substrings,
        )?;
        Self::check_one(
            phase,
            "literal_bytes",
            complexity.literal_bytes,
            self.max_literal_bytes,
        )?;
        Self::check_one(
            phase,
            "max_regex_bytes",
            complexity.max_regex_bytes,
            self.max_regex_bytes,
        )?;
        Self::check_one(
            phase,
            "total_regex_bytes",
            complexity.total_regex_bytes,
            self.max_total_regex_bytes,
        )?;
        Self::check_one(
            phase,
            "regex_size_estimate",
            complexity.regex_size_estimate,
            self.max_regex_size_estimate,
        )?;
        Self::check_one(
            phase,
            "total_regex_size_estimate",
            complexity.total_regex_size_estimate,
            self.max_total_regex_size_estimate,
        )?;
        Self::check_one(
            phase,
            "simplification_expansion",
            complexity.simplification_expansion,
            self.max_simplification_expansion,
        )
    }

    pub(crate) fn check_simplified(
        &self,
        complexity: &GrammarComplexity,
    ) -> Result<(), GrammarLimitError> {
        let phase = GrammarPhase::Simplified;
        Self::check_one(
            phase,
            "simplified_productions",
            complexity.simplified_productions,
            self.max_simplified_productions,
        )?;
        Self::check_one(
            phase,
            "simplified_symbols",
            complexity.simplified_symbols,
            self.max_simplified_symbols,
        )?;
        Self::check_one(
            phase,
            "max_symbols_per_production",
            complexity.max_symbols_per_production,
            self.max_symbols_per_production,
        )
    }

    pub(crate) fn check_deadline(
        &self,
        started: std::time::Instant,
        phase: GrammarPhase,
    ) -> Result<(), GrammarLimitError> {
        if let Some(limit) = self.max_compile_millis {
            let observed = usize::try_from(started.elapsed().as_millis()).unwrap_or(usize::MAX);
            if observed >= limit {
                return Err(GrammarLimitError {
                    phase,
                    resource: "compile_millis",
                    observed,
                    limit,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn check_lexical(&self, input: &str) -> Result<(), GrammarLimitError> {
        if self.max_nesting_depth.is_none() && self.max_alternatives_per_rule.is_none() {
            return Ok(());
        }
        let limit = self.max_nesting_depth.unwrap_or(usize::MAX);
        let alternatives_limit = self.max_alternatives_per_rule.unwrap_or(usize::MAX);
        let bytes = input.as_bytes();
        let mut depth = 0usize;
        let mut max_depth = 0usize;
        let mut alternatives = 1usize;
        let mut quote = None;
        let mut escaped = false;
        let mut comment_depth = 0usize;
        let mut index = 0usize;
        while index < bytes.len() {
            let byte = bytes[index];
            let next = bytes.get(index + 1).copied();
            if comment_depth > 0 {
                if byte == b'(' && next == Some(b'*') {
                    comment_depth = comment_depth.saturating_add(1);
                    index += 2;
                    continue;
                }
                if byte == b'*' && next == Some(b')') {
                    comment_depth = comment_depth.saturating_sub(1);
                    index += 2;
                    continue;
                }
                index += 1;
                continue;
            }
            if let Some(active_quote) = quote {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == active_quote {
                    quote = None;
                }
                index += 1;
                continue;
            }
            if byte == b'(' && next == Some(b'*') {
                comment_depth = 1;
                index += 2;
                continue;
            }
            if byte == b'\'' || byte == b'"' {
                quote = Some(byte);
            } else if byte == b'(' {
                depth = depth.saturating_add(1);
                max_depth = max_depth.max(depth);
                if max_depth > limit {
                    return Err(GrammarLimitError {
                        phase: GrammarPhase::Source,
                        resource: "nesting_depth",
                        observed: max_depth,
                        limit,
                    });
                }
            } else if byte == b')' {
                depth = depth.saturating_sub(1);
            } else if byte == b'|' {
                alternatives = alternatives.saturating_add(1);
                if alternatives > alternatives_limit {
                    return Err(GrammarLimitError {
                        phase: GrammarPhase::Source,
                        resource: "alternatives_per_rule",
                        observed: alternatives,
                        limit: alternatives_limit,
                    });
                }
            } else if byte == b';' {
                alternatives = 1;
            }
            index += 1;
        }
        Ok(())
    }
}

/// Looks up whether a `Nonterminal` node refers to a nullable nonterminal.
pub(crate) trait NullableLookup<S> {
    fn is_nullable_reference(&self, nullable: &ahash::AHashSet<S>) -> bool;
}

impl NullableLookup<string_interner::symbol::SymbolU32> for NodeWithID {
    fn is_nullable_reference(
        &self,
        nullable: &ahash::AHashSet<string_interner::symbol::SymbolU32>,
    ) -> bool {
        matches!(self, NodeWithID::Nonterminal(id) if nullable.contains(id))
    }
}

/// Whether `root` can derive the empty string, given the nonterminals already known nullable.
fn node_nullable(
    root: &NodeWithID,
    grammar: &ParsedGrammar,
    nullable: &ahash::AHashSet<string_interner::symbol::SymbolU32>,
) -> bool {
    enum Visit<'a> {
        Enter(&'a NodeWithID),
        Exit(&'a NodeWithID),
    }
    let mut visits = vec![Visit::Enter(root)];
    let mut values: Vec<bool> = Vec::new();
    while let Some(visit) = visits.pop() {
        match visit {
            Visit::Enter(node) => {
                visits.push(Visit::Exit(node));
                match node {
                    NodeWithID::Multiple(nodes) => visits.extend(nodes.iter().rev().map(Visit::Enter)),
                    NodeWithID::RegexExt(node, _) | NodeWithID::Group(node) => visits.push(Visit::Enter(node)),
                    NodeWithID::Symbol(lhs, _, rhs) => {
                        visits.push(Visit::Enter(rhs));
                        visits.push(Visit::Enter(lhs));
                    }
                    _ => {}
                }
            }
            Visit::Exit(node) => match node {
                NodeWithID::Multiple(nodes) => {
                    let start = values.len().saturating_sub(nodes.len());
                    let value = values[start..].iter().all(|nullable| *nullable);
                    values.truncate(start);
                    values.push(value);
                }
                NodeWithID::RegexExt(_, kind) => {
                    let child = values.pop().unwrap_or(false);
                    values.push(match kind {
                        RegexExtKind::Optional | RegexExtKind::Repeat0 => true,
                        RegexExtKind::Repeat1 => child,
                    });
                }
                NodeWithID::Group(_) => {}
                NodeWithID::Symbol(_, kind, _) => {
                    let rhs = values.pop().unwrap_or(false);
                    let lhs = values.pop().unwrap_or(false);
                    values.push(match kind {
                        SymbolKind::Concatenation => lhs && rhs,
                        SymbolKind::Alternation => lhs || rhs,
                    });
                }
                NodeWithID::Nonterminal(id) => values.push(nullable.contains(id)),
                NodeWithID::Terminal(id) => values.push(
                    grammar
                        .interned_strings
                        .terminals
                        .resolve(*id)
                        .is_some_and(str::is_empty),
                ),
                _ => values.push(false),
            },
        }
    }
    values.pop().unwrap_or(false)
}

/// Computes the nullable nonterminals of a parsed grammar by fixpoint iteration.
fn nullable_nonterminals(
    grammar: &ParsedGrammar,
) -> ahash::AHashSet<string_interner::symbol::SymbolU32> {
    let mut nullable = ahash::AHashSet::default();
    let mut changed = true;
    let mut rounds = 0usize;
    while changed && rounds <= grammar.expressions.len() {
        changed = false;
        rounds += 1;
        for expression in &grammar.expressions {
            if !nullable.contains(&expression.lhs) && node_nullable(&expression.rhs, grammar, &nullable) {
                nullable.insert(expression.lhs);
                changed = true;
            }
        }
    }
    nullable
}
