//! The parsed regex AST and its lowering into an NFA.
//!
//! Parsing builds a [`RegexIr`] on a stack of [`RegexStackEntry`]s; building lowers it into an
//! [`FsmWithStartEnd`] through the `concat`/`union`/`star`/`plus`/`optional` algebra.

use super::{
    CodepointRange,
    fsm::Fsm,
    fsm_with_start_end::FsmWithStartEnd,
    packed_utf8::{MAX_1_BYTE_UNICODE, add_packed_utf8_range_edges, codepoint_to_packed_utf8},
    regex_escape::{MAX_CODEPOINT, RegexEscape},
    regex_node::RegexNode,
    regex_stack_entry::RegexStackEntry,
};
use crate::{grammar::GrammarBuilder, support::parse_next_utf8};

const REPEAT_NO_UPPER_BOUND: i32 = -1;
/// Bounded repetitions above this are compiled into a repeat FSM edge (when a grammar builder is
/// available) instead of being unrolled. Matches the grammar-level repetition expander.
const LARGE_REPEAT_THRESHOLD: i32 = 128;
/// The cap of the estimated state count of a bounded repetition that has to be unrolled.
const MAX_UNROLLED_REPEAT_STATES: i64 = 100_000;
/// `10^9` fits in an `i32`; longer counts could overflow.
const MAX_REPEAT_DIGITS: usize = 9;

/// A parsed regex: the top-level sequence and the flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegexIr {
    states: Vec<RegexNode>,
    case_insensitive: bool,
}

impl RegexIr {
    /// Parses a regex. A leading `(?i)` enables ASCII case-insensitive matching.
    ///
    /// With a `builder`, a bounded repetition whose upper bound (or, if unbounded, lower bound)
    /// exceeds 128 becomes a new rule (with a regex body holding the repeated sub-pattern, named
    /// after `rule_hint`) referenced by a repeat edge, instead of being unrolled.
    ///
    /// # Errors
    /// Returns `"Regex parsing error at position <1-based>: <description>"`; the position
    /// includes the `(?i)` prefix.
    pub fn parse(
        regex_with_flags: &[u8],
        mut builder: Option<&mut GrammarBuilder>,
        rule_hint: &str,
    ) -> Result<Self, String> {
        let (regex, case_insensitive) = match regex_with_flags.strip_prefix(b"(?i)") {
            Some(regex) => (regex, true),
            None => (regex_with_flags, false),
        };
        let flag_prefix_length = regex_with_flags.len() - regex.len();
        let error_at = |pos: usize, message: &str| {
            format!("Regex parsing error at position {}: {message}", pos + flag_prefix_length + 1)
        };
        let lossy = || String::from_utf8_lossy(regex);

        let size = regex.len();
        let mut stack: Vec<RegexStackEntry> = Vec::new();
        let mut i = 0;
        while i < size {
            let c = regex[i];
            match c {
                b'^' | b'$' => {
                    if !((c == b'^' && i == 0) || (c == b'$' && i == size - 1)) {
                        eprintln!("Anchor '{}' in the middle of regex is ignored: {}", char::from(c), lossy());
                    }
                },
                b'[' => {
                    let class_end = skip_character_class(regex, i).ok_or_else(|| error_at(i, "Unclosed '['"))?;
                    let content_begin = if regex.get(i + 1) == Some(&b'^') {
                        i + 2
                    } else {
                        i + 1
                    };
                    if content_begin + 1 == class_end {
                        return Err(error_at(i, "Empty character class is not allowed in regex"));
                    }
                    stack.push(RegexStackEntry::Node {
                        node: RegexNode::Leaf(regex[i..class_end].to_vec()),
                        span_begin: i,
                        span_end: class_end,
                    });
                    i = class_end - 1;
                },
                b']' => return Err(error_at(i, "Unmatched ']'")),
                b'+' | b'*' | b'?' => {
                    let Some(RegexStackEntry::Node {
                        node,
                        span_begin,
                        ..
                    }) = stack.pop()
                    else {
                        return Err(error_at(i, &format!("There is nothing to repeat before '{}'", char::from(c))));
                    };
                    let child = Box::new(node);
                    let node = match c {
                        b'+' => RegexNode::Plus(child),
                        b'*' => RegexNode::Star(child),
                        _ => RegexNode::Optional(child),
                    };
                    // Greedy and non-greedy quantifiers accept the same language.
                    if regex.get(i + 1) == Some(&b'?') {
                        i += 1;
                    }
                    stack.push(RegexStackEntry::Node {
                        node,
                        span_begin,
                        span_end: i + 1,
                    });
                },
                b'(' if regex.get(i + 1) == Some(&b'?') => {
                    let Some(&modifier) = regex.get(i + 2) else {
                        return Err(error_at(i, "Group modifier is not finished"));
                    };
                    match modifier {
                        b':' => {
                            stack.push(RegexStackEntry::GroupOpen(i));
                            i += 2;
                        },
                        b'=' | b'!' => {
                            // Skip the whole lookahead group and treat it as the empty string.
                            eprintln!("Lookahead assertion is not supported and is ignored in regex: {}", lossy());
                            let mut depth = 1;
                            let mut j = i + 3;
                            while j < size && depth > 0 {
                                match regex[j] {
                                    b'\\' => j += 1,
                                    b'[' => {
                                        j = skip_character_class(regex, j)
                                            .ok_or_else(|| error_at(j, "Unclosed '['"))?;
                                        continue;
                                    },
                                    b'(' => depth += 1,
                                    b')' => depth -= 1,
                                    _ => {},
                                }
                                j += 1;
                            }
                            if depth != 0 {
                                return Err(error_at(i, "The parenthesis is not closed"));
                            }
                            stack.push(RegexStackEntry::Node {
                                node: RegexNode::Leaf(Vec::new()),
                                span_begin: i,
                                span_end: j,
                            });
                            i = j - 1;
                        },
                        b'<' | b'P' if modifier == b'<' || regex.get(i + 3) == Some(&b'<') => {
                            let name_begin = if modifier == b'<' {
                                i + 3
                            } else {
                                i + 4
                            };
                            if matches!(regex.get(name_begin), Some(b'=' | b'!')) {
                                return Err(error_at(i, "Lookbehind assertion is not supported in regex"));
                            }
                            let name_end = name_begin
                                + regex[name_begin..]
                                    .iter()
                                    .take_while(|&&c| c.is_ascii_alphanumeric() || c == b'_')
                                    .count();
                            if name_end == name_begin || regex.get(name_end) != Some(&b'>') {
                                return Err(error_at(i, "Invalid named capturing group"));
                            }
                            // The group name is ignored; the content compiles as a plain group.
                            stack.push(RegexStackEntry::GroupOpen(i));
                            i = name_end;
                        },
                        _ => {
                            return Err(error_at(
                                i,
                                &format!("Unsupported group modifier '(?{}'", char::from(modifier)),
                            ));
                        },
                    }
                },
                b'(' => stack.push(RegexStackEntry::GroupOpen(i)),
                b'|' => stack.push(RegexStackEntry::Alternation),
                b')' => {
                    let (open, group_begin) = stack
                        .iter()
                        .enumerate()
                        .rev()
                        .find_map(|(index, entry)| match entry {
                            RegexStackEntry::GroupOpen(begin) => Some((index, *begin)),
                            _ => None,
                        })
                        .ok_or_else(|| error_at(i, "Unmatched ')'"))?;
                    let content = stack.split_off(open + 1);
                    stack.truncate(open);
                    let mut branches =
                        split_branches(content).map_err(|begin| error_at(begin, "The parenthesis is not closed"))?;
                    // An empty group or an empty alternative matches the empty string.
                    let node = if branches.len() == 1 {
                        RegexNode::Bracket(branches.pop().unwrap_or_default())
                    } else {
                        RegexNode::Union(branches.into_iter().map(RegexNode::Bracket).collect())
                    };
                    stack.push(RegexStackEntry::Node {
                        node,
                        span_begin: group_begin,
                        span_end: i + 1,
                    });
                },
                b'{' => {
                    let Some(RegexStackEntry::Node {
                        node,
                        span_begin,
                        span_end,
                    }) = stack.pop()
                    else {
                        return Err(error_at(i, "There is nothing to repeat before the repetition"));
                    };
                    let repeat_begin = i;
                    let (mut lower, upper) =
                        check_repeat(regex, &mut i).map_err(|message| error_at(repeat_begin, &message))?;
                    if regex.get(i + 1) == Some(&b'?') {
                        i += 1;
                    }
                    let is_large_repeat = if upper == REPEAT_NO_UPPER_BOUND {
                        lower > LARGE_REPEAT_THRESHOLD
                    } else {
                        upper > LARGE_REPEAT_THRESHOLD
                    };
                    let node = match builder.as_deref_mut() {
                        Some(builder) if is_large_repeat => {
                            // The repeated sub-pattern becomes a rule with a regex body, repeated through
                            // a repeat edge that the Earley parser counts at runtime.
                            let mut inner_regex = if case_insensitive {
                                b"(?i)".to_vec()
                            } else {
                                Vec::new()
                            };
                            inner_regex.extend_from_slice(&regex[span_begin..span_end]);
                            if node.is_nullable() {
                                // The count of a nullable element cannot be enforced (as in the
                                // repetition normalizer), so the lower bound is relaxed to 0.
                                lower = 0;
                            }
                            let rule_hint = if rule_hint.is_empty() {
                                "regex"
                            } else {
                                rule_hint
                            };
                            let body = builder.add_regex(&inner_regex, false);
                            let rule_id = builder.add_rule_with_hint(&format!("{rule_hint}_repeat"), body);
                            builder.update_lookahead_exact(rule_id, true);
                            if upper != REPEAT_NO_UPPER_BOUND {
                                RegexNode::RepeatSubrule {
                                    rule_id,
                                    lower,
                                    upper,
                                }
                            } else {
                                // {n,} == {n}{0,}: a repeat edge for the mandatory part, then a starred
                                // rule reference.
                                let star = RegexNode::Star(Box::new(RegexNode::RuleRef(rule_id)));
                                if lower > 0 {
                                    RegexNode::Bracket(vec![
                                        RegexNode::RepeatSubrule {
                                            rule_id,
                                            lower,
                                            upper: lower,
                                        },
                                        star,
                                    ])
                                } else {
                                    star
                                }
                            }
                        },
                        _ => RegexNode::Repeat {
                            child: Box::new(node),
                            lower,
                            upper,
                        },
                    };
                    stack.push(RegexStackEntry::Node {
                        node,
                        span_begin,
                        span_end: i + 1,
                    });
                },
                // Each leaf holds exactly one character or escape sequence, so that a following
                // quantifier applies to the whole character.
                b'\\' => {
                    let mut escape_end = i;
                    RegexEscape::parse(regex, &mut escape_end, false).map_err(|message| error_at(i, &message))?;
                    stack.push(RegexStackEntry::Node {
                        node: RegexNode::Leaf(regex[i..escape_end].to_vec()),
                        span_begin: i,
                        span_end: escape_end,
                    });
                    i = escape_end - 1;
                },
                _ => {
                    let num_bytes = parse_next_utf8(&regex[i..]).map_or(1, |(_, num_bytes)| num_bytes);
                    stack.push(RegexStackEntry::Node {
                        node: RegexNode::Leaf(regex[i..i + num_bytes].to_vec()),
                        span_begin: i,
                        span_end: i + num_bytes,
                    });
                    i += num_bytes - 1;
                },
            }
            i += 1;
        }

        let mut branches = split_branches(stack).map_err(|begin| error_at(begin, "The parenthesis is not closed"))?;
        let states = if branches.len() == 1 {
            branches.pop().unwrap_or_default()
        } else {
            vec![RegexNode::Union(branches.into_iter().map(RegexNode::Bracket).collect())]
        };
        Ok(Self {
            states,
            case_insensitive,
        })
    }

    /// Whether the regex matches the empty string.
    #[must_use]
    pub fn matches_empty(&self) -> bool {
        RegexNode::is_nullable_sequence(&self.states)
    }

    /// Lowers the regex into an NFA.
    ///
    /// # Errors
    /// Returns a message for a malformed leaf or a repetition too large to unroll.
    pub fn build(&self) -> Result<FsmWithStartEnd, String> {
        self.build_sequence(&self.states)
    }

    fn build_sequence(
        &self,
        nodes: &[RegexNode],
    ) -> Result<FsmWithStartEnd, String> {
        if nodes.is_empty() {
            return Ok(empty_string_fsm());
        }
        let mut fsms = nodes.iter().map(|node| self.visit(node)).collect::<Result<Vec<_>, _>>()?;
        Ok(if fsms.len() == 1 {
            fsms.swap_remove(0)
        } else {
            FsmWithStartEnd::concat(&fsms)
        })
    }

    fn visit(
        &self,
        node: &RegexNode,
    ) -> Result<FsmWithStartEnd, String> {
        Ok(match node {
            RegexNode::Leaf(regex) => self.build_leaf(regex)?,
            RegexNode::Star(child) => self.visit(child)?.star(),
            RegexNode::Plus(child) => self.visit(child)?.plus(),
            RegexNode::Optional(child) => self.visit(child)?.optional(),
            RegexNode::Union(branches) => FsmWithStartEnd::union(
                &branches.iter().map(|branch| self.visit(branch)).collect::<Result<Vec<_>, _>>()?,
            ),
            RegexNode::Bracket(children) => self.build_sequence(children)?,
            RegexNode::Repeat {
                child,
                lower,
                upper,
            } => self.build_repeat(child, *lower, *upper)?,
            RegexNode::RuleRef(rule_id) => {
                let mut fsm = Fsm::new(2);
                fsm.add_rule_edge(0, 1, *rule_id);
                FsmWithStartEnd::new(fsm, 0, vec![1], false)
            },
            RegexNode::RepeatSubrule {
                rule_id,
                lower,
                upper,
            } => {
                // The state holding the repeat edge is padded with epsilon edges on both sides, so
                // compositions never add another outgoing edge to it (a repeat-edge state must have
                // no other outgoing edges).
                let mut fsm = Fsm::new(4);
                fsm.add_epsilon_edge(0, 1);
                fsm.add_repeat_edge(1, 2, *rule_id, *lower, *upper);
                fsm.add_epsilon_edge(2, 3);
                FsmWithStartEnd::new(fsm, 0, vec![3], false)
            },
        })
    }

    /// Unrolls `child{lower, upper}` (`upper == -1` is unbounded).
    fn build_repeat(
        &self,
        child: &RegexNode,
        lower: i32,
        upper: i32,
    ) -> Result<FsmWithStartEnd, String> {
        let has_upper_bound = upper != REPEAT_NO_UPPER_BOUND;
        if has_upper_bound && upper == 0 {
            // {0} / {0,0} matches exactly the empty string; the general path below starts from one
            // copy of the child, whose end states stay accepting.
            return Ok(empty_string_fsm());
        }
        let child = self.visit(child)?;
        // Guard against state explosion. With a grammar builder, large repetitions become repeat
        // edges instead and never reach this point.
        let num_copies = i64::from(if has_upper_bound {
            upper
        } else {
            lower.max(1)
        });
        if i64::from(child.num_states()) * num_copies > MAX_UNROLLED_REPEAT_STATES {
            let upper_text = if has_upper_bound {
                upper.to_string()
            } else {
                String::new()
            };
            return Err(format!("The bounded repetition {{{lower},{upper_text}}} is too large to compile into a FSM"));
        }

        let mut result = child.clone();
        // A single copy is accepting when the lower bound is at most 1.
        let mut new_ends: Vec<i32> = if lower <= 1 {
            result.ends().to_vec()
        } else {
            Vec::new()
        };
        if has_upper_bound {
            for i in 2..=upper {
                result = FsmWithStartEnd::concat(&[result, child.clone()]);
                if i >= lower {
                    new_ends.extend_from_slice(result.ends());
                }
            }
        } else {
            for _ in 2..lower {
                result = FsmWithStartEnd::concat(&[result, child.clone()]);
            }
            let Some(&end_of_lower_bound) = result.ends().first() else {
                return Err("Internal error: no end state found in the lower bound FSM".to_owned());
            };
            result = FsmWithStartEnd::concat(&[result, child]);
            for end in result.ends().to_vec() {
                result.fsm_mut().add_epsilon_edge(end, end_of_lower_bound);
            }
        }
        for end in new_ends {
            result.add_end_state(end);
        }
        if lower == 0 {
            // A fresh accepting start state makes zero repetitions match. Making the original start
            // accepting would also accept strings that merely loop back to it inside the first copy.
            let (old_start, new_start) = (result.start(), result.add_state());
            result.fsm_mut().add_epsilon_edge(new_start, old_start);
            result.set_start_state(new_start);
            result.add_end_state(new_start);
        }
        Ok(result)
    }

    /// Builds the FSM of one leaf: the empty string, a character class `[...]`, or a sequence of
    /// literal characters, `.` and escapes.
    fn build_leaf(
        &self,
        regex: &[u8],
    ) -> Result<FsmWithStartEnd, String> {
        let mut result = FsmWithStartEnd::new(Fsm::new(1), 0, Vec::new(), false);
        if regex.first() == Some(&b'[') {
            let ranges = self.parse_character_class(regex)?;
            let end = result.add_state();
            add_codepoint_ranges(result.fsm_mut(), 0, end, &ranges);
            result.add_end_state(end);
            return Ok(result);
        }
        let mut current = 0;
        let mut pos = 0;
        while pos < regex.len() {
            let ranges = if regex[pos] == b'.' {
                pos += 1;
                vec![(0, MAX_CODEPOINT)]
            } else if regex[pos] == b'\\' {
                match RegexEscape::parse(regex, &mut pos, false)? {
                    RegexEscape::Single(codepoint) => self.single_codepoint_ranges(codepoint),
                    RegexEscape::Class {
                        ranges,
                        negated,
                    } => resolve_ranges(ranges, self.case_insensitive, negated),
                }
            } else if let Ok((codepoint, num_bytes)) = parse_next_utf8(&regex[pos..]) {
                pos += num_bytes;
                self.single_codepoint_ranges(codepoint as u32)
            } else {
                // Be permissive with non-UTF-8 patterns: match the raw byte.
                let byte = i32::from(regex[pos]);
                pos += 1;
                let next = result.add_state();
                result.fsm_mut().add_edge(current, next, byte, byte);
                current = next;
                continue;
            };
            let next = result.add_state();
            add_codepoint_ranges(result.fsm_mut(), current, next, &ranges);
            current = next;
        }
        result.add_end_state(current);
        Ok(result)
    }

    /// The ranges accepting one codepoint: the codepoint itself, then its ASCII case-folded
    /// counterpart when matching case-insensitively.
    fn single_codepoint_ranges(
        &self,
        codepoint: u32,
    ) -> Vec<CodepointRange> {
        let mut ranges = vec![(codepoint, codepoint)];
        if self.case_insensitive {
            fold_ascii_case_ranges(&mut ranges);
        }
        ranges
    }

    /// Parses a character class leaf `[...]` into its accepted codepoint ranges. ASCII case
    /// folding (if enabled) is applied before negation.
    fn parse_character_class(
        &self,
        regex: &[u8],
    ) -> Result<Vec<CodepointRange>, String> {
        let lossy = || String::from_utf8_lossy(regex);
        let content_end = regex.len() - 1;
        let negated = regex.get(1) == Some(&b'^') && content_end > 1;
        let mut pos = if negated {
            2
        } else {
            1
        };
        if pos == content_end {
            return Err(format!("Empty character class {} is not allowed in regex", lossy()));
        }
        // One class unit: an escape sequence or a literal (possibly multi-byte) character.
        let parse_unit = |pos: &mut usize| {
            if regex[*pos] == b'\\' {
                return RegexEscape::parse(regex, pos, true);
            }
            let (codepoint, num_bytes) = parse_next_utf8(&regex[*pos..content_end])
                .map_err(|_| format!("Invalid UTF-8 in regex character class {}", lossy()))?;
            *pos += num_bytes;
            Ok(RegexEscape::Single(codepoint as u32))
        };

        let mut ranges = Vec::new();
        while pos < content_end {
            match parse_unit(&mut pos)? {
                // Class escapes like \d cannot be a range endpoint; a following '-' is literal.
                RegexEscape::Class {
                    ranges: class_ranges,
                    negated,
                } => ranges.extend(resolve_ranges(class_ranges, false, negated)),
                RegexEscape::Single(low) if regex.get(pos) == Some(&b'-') && pos + 1 < content_end => {
                    pos += 1;
                    let RegexEscape::Single(high) = parse_unit(&mut pos)? else {
                        return Err(format!("Invalid character range endpoint in regex character class {}", lossy()));
                    };
                    if high < low {
                        return Err(format!(
                            "Invalid character range (lower bound exceeds upper bound) in regex character class {}",
                            lossy()
                        ));
                    }
                    ranges.push((low, high));
                },
                RegexEscape::Single(codepoint) => ranges.push((codepoint, codepoint)),
            }
        }
        Ok(resolve_ranges(ranges, self.case_insensitive, negated))
    }
}

/// The FSM matching exactly the empty string.
fn empty_string_fsm() -> FsmWithStartEnd {
    FsmWithStartEnd::new(Fsm::new(1), 0, vec![0], false)
}

/// Splits stack entries (left to right) at the alternations into branches. Returns the offset of
/// an unclosed `(` if one is found.
fn split_branches(entries: Vec<RegexStackEntry>) -> Result<Vec<Vec<RegexNode>>, usize> {
    let mut branches = Vec::new();
    let mut current = Vec::new();
    for entry in entries {
        match entry {
            RegexStackEntry::Node {
                node,
                ..
            } => current.push(node),
            RegexStackEntry::Alternation => branches.push(std::mem::take(&mut current)),
            RegexStackEntry::GroupOpen(begin) => return Err(begin),
        }
    }
    branches.push(current);
    Ok(branches)
}

/// The offset right after the character class starting at `regex[pos] == b'['`, or `None` if it
/// is not closed.
fn skip_character_class(
    regex: &[u8],
    mut pos: usize,
) -> Option<usize> {
    pos += 1;
    if regex.get(pos) == Some(&b'^') {
        pos += 1;
    }
    while pos < regex.len() {
        match regex[pos] {
            b'\\' => pos += 2,
            b']' => return Some(pos + 1),
            _ => pos += 1,
        }
    }
    None
}

/// Parses a `{n}` / `{n,}` / `{n,m}` repetition starting at `regex[*i] == b'{'`, leaving `*i` at
/// the closing `}`. An unbounded upper bound is `-1`.
fn check_repeat(
    regex: &[u8],
    i: &mut usize,
) -> Result<(i32, i32), String> {
    let skip_spaces = |i: &mut usize| {
        while regex.get(*i) == Some(&b' ') {
            *i += 1;
        }
    };
    let parse_count = |i: &mut usize| {
        let digits_begin = *i;
        while regex.get(*i).is_some_and(u8::is_ascii_digit) {
            *i += 1;
        }
        let digits = &regex[digits_begin..*i];
        if digits.len() > MAX_REPEAT_DIGITS {
            return Err(format!(
                "Invalid repetition count: the count {} is too large",
                String::from_utf8_lossy(digits)
            ));
        }
        Ok((!digits.is_empty()).then(|| digits.iter().fold(0, |value, &digit| value * 10 + i32::from(digit - b'0'))))
    };

    *i += 1;
    skip_spaces(i);
    let lower = parse_count(i)?.ok_or("Invalid repetition count: expected a number after '{'")?;
    skip_spaces(i);
    match regex.get(*i) {
        Some(b'}') => return Ok((lower, lower)),
        Some(b',') => *i += 1,
        _ => return Err("Invalid repetition count: expected ',' or '}' after the lower bound".to_owned()),
    }
    skip_spaces(i);
    if regex.get(*i) == Some(&b'}') {
        return Ok((lower, REPEAT_NO_UPPER_BOUND));
    }
    let upper = parse_count(i)?.ok_or("Invalid repetition count: expected a number or '}' after ','")?;
    if upper < lower {
        return Err(format!(
            "Invalid repetition count: the lower bound {lower} is larger than the upper bound {upper}"
        ));
    }
    skip_spaces(i);
    if regex.get(*i) != Some(&b'}') {
        return Err("Invalid repetition count: expected '}' after the upper bound".to_owned());
    }
    Ok((lower, upper))
}

/// Applies ASCII case folding (if `fold_case`) and normalization, then complements the ranges
/// (if `negated`).
fn resolve_ranges(
    mut ranges: Vec<CodepointRange>,
    fold_case: bool,
    negated: bool,
) -> Vec<CodepointRange> {
    if fold_case {
        fold_ascii_case_ranges(&mut ranges);
    }
    ranges.sort_unstable();
    let mut normalized: Vec<CodepointRange> = Vec::with_capacity(ranges.len());
    for (low, high) in ranges {
        match normalized.last_mut() {
            Some(last) if low <= last.1 + 1 => last.1 = last.1.max(high),
            _ => normalized.push((low, high)),
        }
    }
    if !negated {
        return normalized;
    }
    // The complement over the codepoint domain [0, MAX_CODEPOINT].
    let mut complement = Vec::new();
    let mut next = 0;
    for (low, high) in normalized {
        if low > next {
            complement.push((next, low - 1));
        }
        if high >= MAX_CODEPOINT {
            return complement;
        }
        next = next.max(high + 1);
    }
    complement.push((next, MAX_CODEPOINT));
    complement
}

/// Appends the ASCII case-folded counterparts of every letter contained in the ranges.
fn fold_ascii_case_ranges(ranges: &mut Vec<CodepointRange>) {
    const CASE_OFFSET: u32 = 'a' as u32 - 'A' as u32;
    for index in 0..ranges.len() {
        let (low, high) = ranges[index];
        let (lower_low, lower_high) = (low.max('a' as u32), high.min('z' as u32));
        if lower_low <= lower_high {
            ranges.push((lower_low - CASE_OFFSET, lower_high - CASE_OFFSET));
        }
        let (upper_low, upper_high) = (low.max('A' as u32), high.min('Z' as u32));
        if upper_low <= upper_high {
            ranges.push((upper_low + CASE_OFFSET, upper_high + CASE_OFFSET));
        }
    }
}

/// Adds edges from `from` to `to` accepting the UTF-8 encoding of every codepoint in `ranges`.
/// Multi-byte characters get intermediate states.
fn add_codepoint_ranges(
    fsm: &mut Fsm,
    from: i32,
    to: i32,
    ranges: &[CodepointRange],
) {
    for &(low, high) in ranges {
        if low <= MAX_1_BYTE_UNICODE {
            fsm.add_edge(from, to, low as i32, high.min(MAX_1_BYTE_UNICODE) as i32);
        }
        if high > MAX_1_BYTE_UNICODE {
            let low = low.max(MAX_1_BYTE_UNICODE + 1);
            add_packed_utf8_range_edges(fsm, from, to, codepoint_to_packed_utf8(low), codepoint_to_packed_utf8(high));
        }
    }
}
