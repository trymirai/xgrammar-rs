//! Builds an NFA from a (byte-oriented) regex — a port of `RegexIR` and `RegexFSMBuilder`
//! in `cpp/fsm_builder.cc`.
//!
//! The regex is parsed onto a stack into a [`RegexNode`] tree (Thompson construction), then
//! lowered into an [`FsmWithStartEnd`] via the `concat`/`union`/`star`/`plus`/`optional`
//! algebra. This is the FSM-level regex (used to compile grammars), distinct from the
//! regex→EBNF converter.

use super::{fsm::Fsm, fsm_with_start_end::FsmWithStartEnd};

const REPEAT_NO_UPPER_BOUND: i32 = -1;

#[derive(Debug, Clone, Copy)]
enum RegexSymbol {
    Star,
    Plus,
    Optional,
}

/// A node of the parsed regex tree.
#[derive(Debug, Clone)]
enum RegexNode {
    /// A literal string or character class (the raw regex bytes).
    Leaf(Vec<u8>),
    /// A quantified child (`*`, `+`, `?`).
    Symbol(RegexSymbol, Box<RegexNode>),
    /// An alternation of children.
    Union(Vec<RegexNode>),
    /// A concatenation of children.
    Bracket(Vec<RegexNode>),
    /// A `{lower, upper}` repetition of a child (`upper == -1` is unbounded).
    Repeat {
        lower: i32,
        upper: i32,
        child: Box<RegexNode>,
    },
}

/// Builds an FSM accepting the language of `regex`.
///
/// # Errors
/// Returns a message if the regex is malformed.
pub fn build_regex_fsm(regex: &str) -> Result<FsmWithStartEnd, String> {
    let states = parse_regex(regex.as_bytes())?;
    build_ir(&states)
}

fn build_ir(states: &[RegexNode]) -> Result<FsmWithStartEnd, String> {
    if states.is_empty() {
        return Ok(FsmWithStartEnd::new(Fsm::new(1), 0, vec![true], false));
    }
    let mut fsm_list = Vec::with_capacity(states.len());
    for s in states {
        fsm_list.push(visit(s)?);
    }
    if fsm_list.len() > 1 {
        Ok(FsmWithStartEnd::concat(&fsm_list))
    } else {
        Ok(fsm_list.into_iter().next().unwrap())
    }
}

fn visit(node: &RegexNode) -> Result<FsmWithStartEnd, String> {
    match node {
        RegexNode::Leaf(regex) => Ok(build_leaf_fsm(regex)),
        RegexNode::Union(children) => {
            let mut fsm_list = Vec::with_capacity(children.len());
            for c in children {
                fsm_list.push(visit(c)?);
            }
            if fsm_list.len() <= 1 {
                return Err("Invalid union".to_owned());
            }
            Ok(FsmWithStartEnd::union(&fsm_list))
        },
        RegexNode::Bracket(children) => {
            let mut fsm_list = Vec::with_capacity(children.len());
            for c in children {
                fsm_list.push(visit(c)?);
            }
            if fsm_list.is_empty() {
                return Err("Invalid bracket".to_owned());
            }
            Ok(FsmWithStartEnd::concat(&fsm_list))
        },
        RegexNode::Symbol(symbol, child) => {
            let child = visit(child)?;
            Ok(match symbol {
                RegexSymbol::Plus => child.plus(),
                RegexSymbol::Star => child.star(),
                RegexSymbol::Optional => child.optional(),
            })
        },
        RegexNode::Repeat {
            lower,
            upper,
            child,
        } => visit_repeat(*lower, *upper, child),
    }
}

fn visit_repeat(
    lower: i32,
    upper: i32,
    child: &RegexNode,
) -> Result<FsmWithStartEnd, String> {
    let child = visit(child)?;
    let mut result = child.copy();
    let mut new_ends: Vec<i32> = Vec::new();

    if lower == 1 {
        for end in 0..result.num_states() {
            if result.is_end_state(end) {
                new_ends.push(end);
            }
        }
    }

    // {n,}
    if upper == REPEAT_NO_UPPER_BOUND {
        for _ in 2..lower {
            result = FsmWithStartEnd::concat(&[result, child.clone()]);
        }
        let mut end_state_of_lower = -1;
        for end in 0..result.num_states() {
            if result.is_end_state(end) {
                end_state_of_lower = end;
                break;
            }
        }
        result = FsmWithStartEnd::concat(&[result, child]);
        for end in 0..result.num_states() {
            if result.is_end_state(end) {
                result.fsm_mut().add_epsilon_edge(end, end_state_of_lower);
            }
        }
        return Ok(result);
    }

    // {n, m} or {n}
    for i in 2..=upper {
        result = FsmWithStartEnd::concat(&[result, child.clone()]);
        if i >= lower {
            for end in 0..result.num_states() {
                if result.is_end_state(end) {
                    new_ends.push(end);
                }
            }
        }
    }
    for end in new_ends {
        result.add_end_state(end);
    }
    Ok(result)
}

/// Lowers a single literal/class regex into a leaf FSM (no operators).
fn build_leaf_fsm(regex: &[u8]) -> FsmWithStartEnd {
    let mut result = FsmWithStartEnd::new(Fsm::default(), 0, Vec::new(), true);

    if !(regex[0] == b'[' && regex[regex.len() - 1] == b']') {
        // A literal string (possibly with escapes), one transition per element.
        result.add_state();
        let mut i = 0;
        while i < regex.len() {
            if regex[i] != b'\\' {
                let n = result.num_states();
                if regex[i] == b'.' {
                    result.fsm_mut().add_edge(n - 1, n, 0, 0xFF);
                } else {
                    result.fsm_mut().add_edge(
                        n - 1,
                        n,
                        i32::from(regex[i]),
                        i32::from(regex[i]),
                    );
                }
                result.add_state();
                i += 1;
                continue;
            }
            for (lo, hi) in handle_escapes(regex, i) {
                let n = result.num_states();
                result.fsm_mut().add_edge(n - 1, n, lo, hi);
            }
            result.add_state();
            i += 2;
        }
        let n = result.num_states();
        result.add_end_state(n - 1);
        return result;
    }

    // A character class `[...]`.
    result.add_state();
    result.add_state();
    result.add_end_state(1);
    let reverse = regex[1] == b'^';
    let mut i = if reverse {
        2
    } else {
        1
    };
    while i < regex.len() - 1 {
        if regex[i] != b'\\' {
            let is_range = i + 2 < regex.len() - 1 && regex[i + 1] == b'-';
            if !is_range {
                result.fsm_mut().add_edge(
                    0,
                    1,
                    i32::from(regex[i]),
                    i32::from(regex[i]),
                );
                i += 1;
                continue;
            }
            if regex[i + 2] != b'\\' {
                result.fsm_mut().add_edge(
                    0,
                    1,
                    i32::from(regex[i]),
                    i32::from(regex[i + 2]),
                );
                i += 3;
                continue;
            }
            let escaped = handle_escapes(regex, i + 2);
            if escaped.len() != 1 || escaped[0].0 != escaped[0].1 {
                result.fsm_mut().add_edge(
                    0,
                    1,
                    i32::from(regex[i]),
                    i32::from(regex[i]),
                );
                i += 1;
                continue;
            }
            // Note: mirrors the C++ which uses regex[0] (the '[') as the range lower bound.
            result.fsm_mut().add_edge(0, 1, i32::from(regex[0]), escaped[0].0);
            i += 4;
            continue;
        }
        let escaped = handle_escapes(regex, i);
        i += 2;
        if escaped.len() != 1 || escaped[0].0 != escaped[0].1 {
            for (lo, hi) in escaped {
                result.fsm_mut().add_edge(0, 1, lo, hi);
            }
            continue;
        }
        let is_range = i + 1 < regex.len() - 1 && regex[i] == b'-';
        if !is_range {
            result.fsm_mut().add_edge(0, 1, escaped[0].0, escaped[0].1);
            continue;
        }
        if regex[i + 1] != b'\\' {
            result.fsm_mut().add_edge(
                0,
                1,
                escaped[0].0,
                i32::from(regex[i + 1]),
            );
            i += 2;
            continue;
        }
        let rhs = handle_escapes(regex, i + 1);
        if rhs.len() != 1 || rhs[0].0 != rhs[0].1 {
            result.fsm_mut().add_edge(0, 1, escaped[0].0, escaped[0].1);
            continue;
        }
        result.fsm_mut().add_edge(0, 1, escaped[0].0, rhs[0].0);
        i += 3;
    }

    // Simplify the per-character edges into merged ranges (respecting negation).
    let mut has_edge = [false; 0x100];
    for e in result.fsm().state_edges(0) {
        for c in e.min..=e.max {
            has_edge[c as usize] = true;
        }
    }
    let mut new_fsm = Fsm::new(2);
    let mut last: i32 = -1;
    for c in 0..0x100i32 {
        let present = has_edge[c as usize];
        if present != reverse {
            // A character to include in a range (present for normal, absent for reverse).
            if last == -1 {
                last = c;
            }
        } else if last != -1 {
            new_fsm.add_edge(0, 1, last, c - 1);
            last = -1;
        }
    }
    if last != -1 {
        new_fsm.add_edge(0, 1, last, 0xFF);
    }
    FsmWithStartEnd::new(new_fsm, 0, vec![false, true], false)
}

/// Expands an escape sequence at `start` (where `regex[start] == b'\\'`) into one or more
/// inclusive `(min, max)` character ranges.
fn handle_escapes(
    regex: &[u8],
    start: usize,
) -> Vec<(i32, i32)> {
    match regex[start + 1] {
        b'n' => vec![(i32::from(b'\n'), i32::from(b'\n'))],
        b't' => vec![(i32::from(b'\t'), i32::from(b'\t'))],
        b'r' => vec![(i32::from(b'\r'), i32::from(b'\r'))],
        b'0' => vec![(0, 0)],
        b's' => vec![(0, i32::from(b' '))],
        b'S' => vec![(i32::from(b' ') + 1, 0x00FF)],
        b'd' => vec![(i32::from(b'0'), i32::from(b'9'))],
        b'D' => vec![(0, i32::from(b'0') - 1), (i32::from(b'9') + 1, 0x00FF)],
        b'w' => vec![
            (i32::from(b'0'), i32::from(b'9')),
            (i32::from(b'a'), i32::from(b'z')),
            (i32::from(b'A'), i32::from(b'Z')),
            (i32::from(b'_'), i32::from(b'_')),
        ],
        b'W' => vec![
            (0, i32::from(b'0') - 1),
            (i32::from(b'9') + 1, i32::from(b'A') - 1),
            (i32::from(b'Z') + 1, i32::from(b'_') - 1),
            (i32::from(b'_') + 1, i32::from(b'a') - 1),
            (i32::from(b'z') + 1, 0x00FF),
        ],
        other => vec![(i32::from(other), i32::from(other))],
    }
}

/// Parses a `{n}` / `{n,}` / `{n,m}` repetition starting at `*i == b'{'`, advancing `*i` to
/// the closing `}`.
fn check_repeat(
    regex: &[u8],
    i: &mut usize,
) -> Result<(i32, i32), String> {
    if regex[*i] != b'{' {
        return Err("Invalid repeat format1".to_owned());
    }
    *i += 1;
    while *i < regex.len() && regex[*i] == b' ' {
        *i += 1;
    }
    let mut num = String::new();
    while *i < regex.len() && regex[*i].is_ascii_digit() {
        num.push(regex[*i] as char);
        *i += 1;
    }
    if num.is_empty() {
        return Err("Invalid repeat format2".to_owned());
    }
    let lower: i32 =
        num.parse().map_err(|_| "Invalid repeat format2".to_owned())?;
    while *i < regex.len() && regex[*i] == b' ' {
        *i += 1;
    }
    if regex[*i] == b'}' {
        return Ok((lower, lower));
    }
    if regex[*i] != b',' {
        return Err("Invalid repeat format3".to_owned());
    }
    *i += 1;
    while *i < regex.len() && regex[*i] == b' ' {
        *i += 1;
    }
    if regex[*i] == b'}' {
        return Ok((lower, REPEAT_NO_UPPER_BOUND));
    }
    num.clear();
    while *i < regex.len() && regex[*i].is_ascii_digit() {
        num.push(regex[*i] as char);
        *i += 1;
    }
    if num.is_empty() {
        return Err("Invalid repeat format4".to_owned());
    }
    let upper: i32 =
        num.parse().map_err(|_| "Invalid repeat format4".to_owned())?;
    while *i < regex.len() && regex[*i] == b' ' {
        *i += 1;
    }
    if regex[*i] != b'}' {
        return Err("Invalid repeat format5".to_owned());
    }
    Ok((lower, upper))
}

enum StackItem {
    Node(RegexNode),
    Char(u8),
}

#[allow(clippy::too_many_lines)]
fn parse_regex(regex: &[u8]) -> Result<Vec<RegexNode>, String> {
    let len = regex.len();
    let mut stack: Vec<StackItem> = Vec::new();
    let mut left_bracket: i32 = -1;
    let mut i = 0;
    while i < len {
        let c = regex[i];
        if (i == 0 && c == b'^') || (i == len - 1 && c == b'$') {
            i += 1;
            continue;
        }
        if c == b'[' {
            if left_bracket != -1 {
                return Err("Nested middle bracket!".to_owned());
            }
            left_bracket = i as i32;
            i += 1;
            continue;
        }
        if c == b']' {
            if left_bracket == -1 {
                return Err("Invalid middle bracket!".to_owned());
            }
            stack.push(StackItem::Node(RegexNode::Leaf(
                regex[left_bracket as usize..=i].to_vec(),
            )));
            left_bracket = -1;
            i += 1;
            continue;
        }
        if left_bracket != -1 {
            if c == b'\\' {
                i += 1;
            }
            i += 1;
            continue;
        }
        if c == b'+' || c == b'*' || c == b'?' {
            let Some(StackItem::Node(child)) = stack.pop() else {
                return Err(
                    "Invalid regex: no state before operator!".to_owned()
                );
            };
            let symbol = match c {
                b'+' => RegexSymbol::Plus,
                b'*' => RegexSymbol::Star,
                _ => RegexSymbol::Optional,
            };
            stack.push(StackItem::Node(RegexNode::Symbol(
                symbol,
                Box::new(child),
            )));
            i += 1;
            continue;
        }
        if c == b'(' || c == b'|' {
            stack.push(StackItem::Char(c));
            // Skip a non-capturing-group `(?:` or a lookahead `(?!` / `(?=` prefix (lookahead
            // content is currently treated like an ordinary group).
            if c == b'('
                && i + 2 < len
                && regex[i + 1] == b'?'
                && matches!(regex[i + 2], b':' | b'!' | b'=')
            {
                i += 2;
            }
            i += 1;
            continue;
        }
        if c == b')' {
            parse_close_paren(&mut stack)?;
            i += 1;
            continue;
        }
        if c == b'{' {
            let Some(StackItem::Node(child)) = stack.pop() else {
                return Err("Invalid regex: no state before repeat!".to_owned());
            };
            let (lower, upper) = check_repeat(regex, &mut i)?;
            stack.push(StackItem::Node(RegexNode::Repeat {
                lower,
                upper,
                child: Box::new(child),
            }));
            i += 1;
            continue;
        }
        let leaf = if c != b'\\' {
            RegexNode::Leaf(vec![c])
        } else {
            let leaf = RegexNode::Leaf(regex[i..i + 2].to_vec());
            i += 1;
            leaf
        };
        stack.push(StackItem::Node(leaf));
        i += 1;
    }

    drain_stack(stack)
}

/// On `)`, pops the stack to the matching `(`, building a bracket (concatenation) or, if a
/// `|` was seen, a union of brackets.
fn parse_close_paren(stack: &mut Vec<StackItem>) -> Result<(), String> {
    let mut inner: Vec<StackItem> = Vec::new();
    let mut paired = false;
    let mut unioned = false;
    while let Some(item) = stack.pop() {
        match item {
            StackItem::Char(b'(') => {
                paired = true;
                break;
            },
            StackItem::Char(b'|') => {
                unioned = true;
                inner.push(StackItem::Char(b'|'));
            },
            other => inner.push(other),
        }
    }
    if !paired {
        return Err("Invalid regex: no paired bracket!".to_owned());
    }
    if inner.is_empty() {
        return Ok(());
    }
    if !unioned {
        let mut bracket = Vec::new();
        while let Some(item) = inner.pop() {
            let StackItem::Node(child) = item else {
                return Err("Invalid regex: no paired bracket!".to_owned());
            };
            bracket.push(child);
        }
        stack.push(StackItem::Node(RegexNode::Bracket(bracket)));
    } else {
        let mut union_states = Vec::new();
        let mut bracket = Vec::new();
        while let Some(item) = inner.pop() {
            match item {
                StackItem::Char(b'|') => {
                    union_states
                        .push(RegexNode::Bracket(std::mem::take(&mut bracket)));
                },
                StackItem::Node(child) => bracket.push(child),
                StackItem::Char(_) => {
                    return Err("Invalid regex: no paired bracket!".to_owned());
                },
            }
        }
        union_states.push(RegexNode::Bracket(bracket));
        stack.push(StackItem::Node(RegexNode::Union(union_states)));
    }
    Ok(())
}

/// Drains the final stack into the IR's top-level state list, building a union if `|` was
/// seen at the top level.
fn drain_stack(mut stack: Vec<StackItem>) -> Result<Vec<RegexNode>, String> {
    let mut res_states: Vec<RegexNode> = Vec::new();
    let mut union_state_list: Vec<Vec<RegexNode>> = Vec::new();
    let mut unioned = false;
    while let Some(item) = stack.pop() {
        match item {
            StackItem::Char(b'|') => {
                union_state_list.push(std::mem::take(&mut res_states));
                unioned = true;
            },
            StackItem::Char(_) => {
                return Err("Invalid regex: no paired!".to_owned());
            },
            StackItem::Node(child) => res_states.push(child),
        }
    }
    if !unioned {
        res_states.reverse();
        Ok(res_states)
    } else {
        union_state_list.push(res_states);
        let mut union_states = Vec::with_capacity(union_state_list.len());
        for mut group in union_state_list {
            group.reverse();
            union_states.push(RegexNode::Bracket(group));
        }
        Ok(vec![RegexNode::Union(union_states)])
    }
}
