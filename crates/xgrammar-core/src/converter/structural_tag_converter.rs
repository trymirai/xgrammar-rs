//! Resolves, analyzes, and converts a structural-tag [`Format`] tree into a [`Grammar`] —
//! a port of `StructuralTagTokenResolver`, `StructuralTagAnalyzer`, and
//! `StructuralTagGrammarConverter` in `cpp/structural_tag.cc`.

use std::collections::HashMap;

use super::{
    json_schema_converter::json_schema_to_ebnf,
    structural_tag_error::StructuralTagError,
    structural_tag_format::{
        Format, IntOrString, TagBegin, TagEnd, TagFormat,
        TagsWithSeparatorFormat, TokenFormat, TokenTriggeredTagsFormat,
        TriggeredTagsFormat,
    },
    structural_tag_parser::parse_structural_tag,
    xml_tool_calling_converter::xml_tool_calling_to_ebnf,
};
use crate::{
    functor::{add_sub_grammar, grammar_normalizer},
    grammar::{Grammar, GrammarBuilder, TagDispatch, TokenTagDispatch},
    tokenizer::TokenizerInfo,
};

type Ist = StructuralTagError;

impl Grammar {
    /// Builds a grammar from a structural-tag JSON document.
    ///
    /// String-keyed token references require a tokenizer; use
    /// [`Grammar::from_structural_tag_with_tokenizer`] for those (integer token ids work
    /// without one).
    ///
    /// # Errors
    /// Returns a [`StructuralTagError`] if the document is invalid or unsatisfiable.
    pub fn from_structural_tag(
        json: &str
    ) -> Result<Grammar, StructuralTagError> {
        build_structural_tag(json, None)
    }

    /// Builds a grammar from a structural-tag JSON document, resolving string token
    /// references against `tokenizer_info`'s decoded vocabulary (the C++
    /// `Grammar::FromStructuralTag(json, tokenizer_info)`).
    ///
    /// # Errors
    /// Returns a [`StructuralTagError`] if the document is invalid, unsatisfiable, or refers
    /// to a token string absent from the vocabulary.
    pub fn from_structural_tag_with_tokenizer(
        json: &str,
        tokenizer_info: &TokenizerInfo,
    ) -> Result<Grammar, StructuralTagError> {
        build_structural_tag(json, Some(tokenizer_info.decoded_vocab()))
    }
}

fn build_structural_tag(
    json: &str,
    vocab: Option<&[Vec<u8>]>,
) -> Result<Grammar, StructuralTagError> {
    let mut format = parse_structural_tag(json)?;
    resolve_format(&mut format, vocab)?;
    analyze(&mut format, None)?;
    let grammar = StructuralTagConverter::new().convert(&format)?;
    Ok(grammar_normalizer(&grammar))
}

/* ============================ Token resolver ============================ */

fn resolve_token(
    tf: &mut TokenFormat,
    vocab: Option<&[Vec<u8>]>,
) -> Result<(), Ist> {
    if tf.resolved_token_id >= 0 {
        return Ok(());
    }
    let IntOrString::Str(s) = &tf.token else {
        return Ok(());
    };
    let Some(vocab) = vocab else {
        return Err(Ist::invalid(
            "Token string resolution requires tokenizer_info",
        ));
    };
    match vocab.iter().position(|v| v.as_slice() == s.as_bytes()) {
        Some(i) => {
            tf.resolved_token_id = i as i32;
            Ok(())
        },
        None => Err(Ist::invalid(format!(
            "Token string \"{s}\" not found in vocabulary"
        ))),
    }
}

fn resolve_vec(
    input: &[IntOrString],
    vocab: Option<&[Vec<u8>]>,
) -> Result<Vec<i32>, Ist> {
    let mut out = Vec::with_capacity(input.len());
    for item in input {
        match item {
            IntOrString::Int(i) => out.push(*i),
            IntOrString::Str(s) => {
                let Some(vocab) = vocab else {
                    return Err(Ist::invalid(
                        "Token string resolution requires tokenizer_info",
                    ));
                };
                match vocab.iter().position(|v| v.as_slice() == s.as_bytes()) {
                    Some(i) => out.push(i as i32),
                    None => {
                        return Err(Ist::invalid(format!(
                            "Token string \"{s}\" not found in vocabulary"
                        )));
                    },
                }
            },
        }
    }
    Ok(out)
}

fn resolve_tag(
    tag: &mut TagFormat,
    vocab: Option<&[Vec<u8>]>,
) -> Result<(), Ist> {
    if let TagBegin::Token(tf) = &mut tag.begin {
        resolve_token(tf, vocab)?;
    }
    if let TagEnd::Token(tf) = &mut tag.end {
        resolve_token(tf, vocab)?;
    }
    resolve_format(&mut tag.content, vocab)
}

fn resolve_format(
    format: &mut Format,
    vocab: Option<&[Vec<u8>]>,
) -> Result<(), Ist> {
    match format {
        Format::Token(tf) => resolve_token(tf, vocab),
        Format::ExcludeToken(f) => {
            f.resolved_token_ids = resolve_vec(&f.exclude_tokens, vocab)?;
            Ok(())
        },
        Format::AnyTokens(f) => {
            f.resolved_exclude_token_ids =
                resolve_vec(&f.exclude_tokens, vocab)?;
            Ok(())
        },
        Format::TokenTriggeredTags(f) => {
            f.resolved_trigger_token_ids =
                resolve_vec(&f.trigger_tokens, vocab)?;
            f.resolved_exclude_token_ids =
                resolve_vec(&f.exclude_tokens, vocab)?;
            for tag in &mut f.tags {
                resolve_tag(tag, vocab)?;
            }
            Ok(())
        },
        Format::TokenDispatch(f) => {
            let triggers: Vec<IntOrString> =
                f.rules.iter().map(|(t, _)| t.clone()).collect();
            f.resolved_trigger_token_ids = resolve_vec(&triggers, vocab)?;
            f.resolved_exclude_token_ids =
                resolve_vec(&f.exclude_tokens, vocab)?;
            for (_, content) in &mut f.rules {
                resolve_format(content, vocab)?;
            }
            Ok(())
        },
        Format::Dispatch(f) => {
            for (_, content) in &mut f.rules {
                resolve_format(content, vocab)?;
            }
            Ok(())
        },
        Format::Tag(f) => resolve_tag(f, vocab),
        Format::Sequence(f) => {
            for e in &mut f.elements {
                resolve_format(e, vocab)?;
            }
            Ok(())
        },
        Format::Or(f) => {
            for e in &mut f.elements {
                resolve_format(e, vocab)?;
            }
            Ok(())
        },
        Format::TriggeredTags(f) => {
            for tag in &mut f.tags {
                resolve_tag(tag, vocab)?;
            }
            Ok(())
        },
        Format::TagsWithSeparator(f) => {
            for tag in &mut f.tags {
                resolve_tag(tag, vocab)?;
            }
            Ok(())
        },
        Format::Optional(f) => resolve_format(&mut f.content, vocab),
        Format::Plus(f) => resolve_format(&mut f.content, vocab),
        Format::Star(f) => resolve_format(&mut f.content, vocab),
        Format::Repeat(f) => resolve_format(&mut f.content, vocab),
        Format::ConstString(_)
        | Format::JsonSchema(_)
        | Format::AnyText(_)
        | Format::Grammar(_)
        | Format::Regex(_) => Ok(()),
    }
}

/* ============================ Analyzer ============================ */

fn end_strings_of(nearest_tag_end: Option<&TagEnd>) -> Vec<String> {
    match nearest_tag_end {
        Some(TagEnd::Strings(v)) => v.clone(),
        _ => Vec::new(),
    }
}

fn end_token_ids_of(nearest_tag_end: Option<&TagEnd>) -> Vec<i32> {
    match nearest_tag_end {
        Some(TagEnd::Token(tf)) => vec![tf.resolved_token_id],
        _ => Vec::new(),
    }
}

fn is_unlimited(format: &Format) -> bool {
    match format {
        Format::AnyText(_)
        | Format::TriggeredTags(_)
        | Format::TokenTriggeredTags(_)
        | Format::Dispatch(_)
        | Format::TokenDispatch(_)
        | Format::AnyTokens(_)
        | Format::TagsWithSeparator(_)
        | Format::Star(_)
        | Format::Plus(_) => true,
        Format::Sequence(f) => f.is_unlimited,
        Format::Or(f) => f.is_unlimited,
        Format::Optional(f) => is_unlimited(&f.content),
        Format::Repeat(f) => {
            f.max == -1 || (f.max != 0 && is_unlimited(&f.content))
        },
        _ => false,
    }
}

fn is_excluded(format: &Format) -> bool {
    match format {
        Format::AnyText(f) => !f.excludes.is_empty(),
        Format::TriggeredTags(f) => !f.excludes.is_empty(),
        Format::TokenTriggeredTags(f) => !f.exclude_tokens.is_empty(),
        Format::Dispatch(f) => !f.excludes.is_empty(),
        Format::TokenDispatch(f) => !f.exclude_tokens.is_empty(),
        Format::AnyTokens(f) => !f.exclude_tokens.is_empty(),
        _ => false,
    }
}

fn analyze_tag(tag: &mut TagFormat) -> Result<(), Ist> {
    let end = tag.end.clone();
    analyze(&mut tag.content, Some(&end))?;
    if is_unlimited(&tag.content) {
        if let TagEnd::Strings(ends) = &tag.end {
            let has_non_empty = ends.iter().any(|s| !s.is_empty());
            if !has_non_empty && !is_excluded(&tag.content) {
                return Err(Ist::invalid(
                    "When the content is unlimited, at least one end string must be non-empty",
                ));
            }
        }
    }
    Ok(())
}

fn analyze(
    format: &mut Format,
    nearest_tag_end: Option<&TagEnd>,
) -> Result<(), Ist> {
    match format {
        Format::AnyText(f) => {
            f.detected_end_strs = end_strings_of(nearest_tag_end);
            Ok(())
        },
        Format::ExcludeToken(f) => {
            f.detected_end_token_ids = end_token_ids_of(nearest_tag_end);
            Ok(())
        },
        Format::AnyTokens(f) => {
            f.detected_end_token_ids = end_token_ids_of(nearest_tag_end);
            Ok(())
        },
        Format::Sequence(f) => {
            let mut any_unlimited = false;
            for e in &mut f.elements {
                analyze(e, nearest_tag_end)?;
                any_unlimited |= is_unlimited(e) && !is_excluded(e);
            }
            f.is_unlimited = any_unlimited;
            Ok(())
        },
        Format::Or(f) => {
            let mut any_unlimited = false;
            for e in &mut f.elements {
                analyze(e, nearest_tag_end)?;
                any_unlimited |= is_unlimited(e) && !is_excluded(e);
            }
            f.is_unlimited = any_unlimited;
            Ok(())
        },
        Format::Tag(f) => analyze_tag(f),
        Format::TriggeredTags(f) => {
            for tag in &mut f.tags {
                analyze_tag(tag)?;
            }
            f.detected_end_strs = end_strings_of(nearest_tag_end);
            Ok(())
        },
        Format::TagsWithSeparator(f) => {
            for tag in &mut f.tags {
                analyze_tag(tag)?;
            }
            Ok(())
        },
        Format::TokenTriggeredTags(f) => {
            for tag in &mut f.tags {
                analyze_tag(tag)?;
            }
            f.detected_end_token_ids = end_token_ids_of(nearest_tag_end);
            Ok(())
        },
        Format::Optional(f) => analyze(&mut f.content, nearest_tag_end),
        Format::Plus(f) => analyze(&mut f.content, nearest_tag_end),
        Format::Star(f) => analyze(&mut f.content, nearest_tag_end),
        Format::Repeat(f) => analyze(&mut f.content, nearest_tag_end),
        Format::Dispatch(f) => {
            for (_, content) in &mut f.rules {
                analyze(content, nearest_tag_end)?;
            }
            Ok(())
        },
        Format::TokenDispatch(f) => {
            for (_, content) in &mut f.rules {
                analyze(content, nearest_tag_end)?;
            }
            Ok(())
        },
        Format::ConstString(_)
        | Format::JsonSchema(_)
        | Format::Grammar(_)
        | Format::Regex(_)
        | Format::Token(_) => Ok(()),
    }
}

/* ============================ Grammar converter ============================ */

struct StructuralTagConverter {
    builder: GrammarBuilder,
    cache: HashMap<Format, i32>,
}

impl StructuralTagConverter {
    fn new() -> Self {
        Self {
            builder: GrammarBuilder::new(),
            cache: HashMap::new(),
        }
    }

    fn convert(
        mut self,
        format: &Format,
    ) -> Result<Grammar, Ist> {
        let root_ref_rule_id = self.visit(format)?;
        let expr = self.builder.add_rule_ref(root_ref_rule_id);
        let seq = self.builder.add_sequence(&[expr]);
        let choices = self.builder.add_choices(&[seq]);
        let root_rule_id = self.builder.add_rule_with_hint("root", choices);
        Ok(self.builder.into_grammar_with_root_id(root_rule_id))
    }

    fn visit(
        &mut self,
        format: &Format,
    ) -> Result<i32, Ist> {
        if let Some(id) = self.cache.get(format) {
            return Ok(*id);
        }
        let id = self.visit_sub(format)?;
        self.cache.insert(format.clone(), id);
        Ok(id)
    }

    fn build_begin_expr(
        &mut self,
        tag: &TagFormat,
    ) -> i32 {
        match &tag.begin {
            TagBegin::Str(s) => self.builder.add_byte_string(s),
            TagBegin::Token(tf) => {
                self.builder.add_token_set(&[tf.resolved_token_id])
            },
        }
    }

    fn build_end_expr(
        &mut self,
        tag: &TagFormat,
    ) -> i32 {
        match &tag.end {
            TagEnd::Token(tf) => {
                self.builder.add_token_set(&[tf.resolved_token_id])
            },
            TagEnd::Strings(ends) => {
                if ends.len() == 1 {
                    if ends[0].is_empty() {
                        self.builder.add_empty_str()
                    } else {
                        self.builder.add_byte_string(&ends[0])
                    }
                } else {
                    let mut end_seq_ids = Vec::with_capacity(ends.len());
                    for s in ends {
                        let e = if s.is_empty() {
                            self.builder.add_empty_str()
                        } else {
                            self.builder.add_byte_string(s)
                        };
                        end_seq_ids.push(self.builder.add_sequence(&[e]));
                    }
                    let choice = self.builder.add_choices(&end_seq_ids);
                    let rule =
                        self.builder.add_rule_with_hint("tag_end", choice);
                    self.builder.add_rule_ref(rule)
                }
            },
        }
    }

    fn add_tag_dispatch_rule(
        &mut self,
        tag_rule_pairs: Vec<(String, i32)>,
        loop_after_dispatch: bool,
        excludes: Vec<String>,
    ) -> i32 {
        let td = TagDispatch {
            tag_rule_pairs: tag_rule_pairs
                .into_iter()
                .map(|(s, id)| (s.into_bytes(), id))
                .collect(),
            loop_after_dispatch,
            excludes: excludes.into_iter().map(String::into_bytes).collect(),
        };
        self.builder.add_tag_dispatch(&td)
    }

    fn add_token_tag_dispatch_rule(
        &mut self,
        trigger_rule_pairs: Vec<(i32, i32)>,
        loop_after_dispatch: bool,
        excludes: Vec<i32>,
    ) -> i32 {
        let ttd = TokenTagDispatch {
            trigger_rule_pairs,
            loop_after_dispatch,
            excludes,
        };
        self.builder.add_token_tag_dispatch(&ttd)
    }

    #[allow(clippy::too_many_lines)]
    fn visit_sub(
        &mut self,
        format: &Format,
    ) -> Result<i32, Ist> {
        match format {
            Format::ConstString(f) => {
                let expr = if f.value.is_empty() {
                    self.builder.add_empty_str()
                } else {
                    self.builder.add_byte_string(&f.value)
                };
                let seq = self.builder.add_sequence(&[expr]);
                let choices = self.builder.add_choices(&[seq]);
                Ok(self.builder.add_rule_with_hint("const_string", choices))
            },
            Format::JsonSchema(f) => {
                let ebnf = if f.style == "json" {
                    json_schema_to_ebnf(
                        &f.json_schema,
                        true,
                        None,
                        None,
                        true,
                        None,
                    )
                    .map_err(|e| Ist::InvalidJsonSchema(e.to_string()))?
                } else {
                    xml_tool_calling_to_ebnf(&f.json_schema, &f.style)
                        .map_err(|e| Ist::InvalidJsonSchema(e.to_string()))?
                };
                let sub = Grammar::from_ebnf(&ebnf, "root")
                    .map_err(|e| Ist::InvalidJsonSchema(e.to_string()))?;
                Ok(add_sub_grammar(&mut self.builder, &sub))
            },
            Format::Grammar(f) => {
                let sub = Grammar::from_ebnf(&f.grammar, "root")
                    .map_err(|e| Ist::invalid(e.to_string()))?;
                Ok(add_sub_grammar(&mut self.builder, &sub))
            },
            Format::Regex(f) => {
                let sub = Grammar::from_regex(&f.pattern)
                    .map_err(|e| Ist::invalid(e.to_string()))?;
                Ok(add_sub_grammar(&mut self.builder, &sub))
            },
            Format::AnyText(f) => {
                let mut all_excludes = f.excludes.clone();
                all_excludes.extend(
                    f.detected_end_strs
                        .iter()
                        .filter(|s| !s.is_empty())
                        .cloned(),
                );
                if all_excludes.is_empty() {
                    let any = self.builder.add_character_class_star(
                        &[crate::grammar::CharacterClassElement::new(
                            0, 0x10_FFFF,
                        )],
                        false,
                    );
                    let seq = self.builder.add_sequence(&[any]);
                    let choices = self.builder.add_choices(&[seq]);
                    Ok(self.builder.add_rule_with_hint("any_text", choices))
                } else {
                    let td = self.add_tag_dispatch_rule(
                        Vec::new(),
                        false,
                        all_excludes,
                    );
                    Ok(self.builder.add_rule_with_hint("any_text", td))
                }
            },
            Format::Sequence(f) => {
                let mut refs = Vec::with_capacity(f.elements.len());
                for e in &f.elements {
                    let id = self.visit(e)?;
                    refs.push(self.builder.add_rule_ref(id));
                }
                let seq = self.builder.add_sequence(&refs);
                let choices = self.builder.add_choices(&[seq]);
                Ok(self.builder.add_rule_with_hint("sequence", choices))
            },
            Format::Or(f) => {
                let mut seqs = Vec::with_capacity(f.elements.len());
                for e in &f.elements {
                    let id = self.visit(e)?;
                    let r = self.builder.add_rule_ref(id);
                    seqs.push(self.builder.add_sequence(&[r]));
                }
                let choices = self.builder.add_choices(&seqs);
                Ok(self.builder.add_rule_with_hint("or", choices))
            },
            Format::Tag(f) => {
                let content_id = self.visit(&f.content)?;
                let begin = self.build_begin_expr(f);
                let content_ref = self.builder.add_rule_ref(content_id);
                let end = self.build_end_expr(f);
                let seq = self.builder.add_sequence(&[begin, content_ref, end]);
                let choices = self.builder.add_choices(&[seq]);
                Ok(self.builder.add_rule_with_hint("tag", choices))
            },
            Format::TriggeredTags(f) => self.visit_triggered_tags(f),
            Format::TagsWithSeparator(f) => self.visit_tags_with_separator(f),
            Format::Optional(f) => {
                let content_id = self.visit(&f.content)?;
                let content_ref = self.builder.add_rule_ref(content_id);
                let empty = self.builder.add_empty_str();
                let seq = self.builder.add_sequence(&[content_ref]);
                let choices = self.builder.add_choices(&[empty, seq]);
                Ok(self.builder.add_rule_with_hint("optional", choices))
            },
            Format::Plus(f) => {
                let content_id = self.visit(&f.content)?;
                let content_ref = self.builder.add_rule_ref(content_id);
                let star_rule =
                    self.builder.add_empty_rule_with_hint("plus_star");
                let star_ref = self.builder.add_rule_ref(star_rule);
                let empty = self.builder.add_empty_str();
                let inner = self.builder.add_sequence(&[content_ref, star_ref]);
                let star_body = self.builder.add_choices(&[empty, inner]);
                self.builder.update_rule_body(star_rule, star_body);
                let plus = self.builder.add_sequence(&[content_ref, star_ref]);
                Ok(self.builder.add_rule_with_hint("plus", plus))
            },
            Format::Star(f) => {
                let content_id = self.visit(&f.content)?;
                let content_ref = self.builder.add_rule_ref(content_id);
                let star_rule = self.builder.add_empty_rule_with_hint("star");
                let star_ref = self.builder.add_rule_ref(star_rule);
                let empty = self.builder.add_empty_str();
                let inner = self.builder.add_sequence(&[content_ref, star_ref]);
                let star_body = self.builder.add_choices(&[empty, inner]);
                self.builder.update_rule_body(star_rule, star_body);
                Ok(self.builder.add_rule_with_hint("star", star_ref))
            },
            Format::Repeat(f) => {
                let content_id = self.visit(&f.content)?;
                let repeat = self.builder.add_repeat(content_id, f.min, f.max);
                Ok(self.builder.add_rule_with_hint("repeat", repeat))
            },
            Format::Token(f) => {
                let token_set =
                    self.builder.add_token_set(&[f.resolved_token_id]);
                let seq = self.builder.add_sequence(&[token_set]);
                let choices = self.builder.add_choices(&[seq]);
                Ok(self.builder.add_rule_with_hint("token", choices))
            },
            Format::ExcludeToken(f) => {
                let mut all = f.resolved_token_ids.clone();
                all.extend_from_slice(&f.detected_end_token_ids);
                let expr = self.builder.add_exclude_token_set(&all);
                let seq = self.builder.add_sequence(&[expr]);
                let choices = self.builder.add_choices(&[seq]);
                Ok(self.builder.add_rule_with_hint("exclude_token", choices))
            },
            Format::AnyTokens(f) => {
                let mut all = f.resolved_exclude_token_ids.clone();
                all.extend_from_slice(&f.detected_end_token_ids);
                let expr = self.builder.add_exclude_token_set(&all);
                let seq = self.builder.add_sequence(&[expr]);
                let choices = self.builder.add_choices(&[seq]);
                let inner_rule = self
                    .builder
                    .add_rule_with_hint("any_tokens_inner", choices);
                let inner_ref = self.builder.add_rule_ref(inner_rule);
                let star_rule =
                    self.builder.add_empty_rule_with_hint("any_tokens");
                let star_ref = self.builder.add_rule_ref(star_rule);
                let empty = self.builder.add_empty_str();
                let seq2 = self.builder.add_sequence(&[inner_ref, star_ref]);
                let star_body = self.builder.add_choices(&[empty, seq2]);
                self.builder.update_rule_body(star_rule, star_body);
                Ok(star_rule)
            },
            Format::TokenTriggeredTags(f) => self.visit_token_triggered_tags(f),
            Format::Dispatch(f) => {
                let mut pairs = Vec::with_capacity(f.rules.len());
                for (trigger, content) in &f.rules {
                    let id = self.visit(content)?;
                    pairs.push((trigger.clone(), id));
                }
                let expr = self.add_tag_dispatch_rule(
                    pairs,
                    f.loop_after_dispatch,
                    f.excludes.clone(),
                );
                Ok(self.builder.add_rule_with_hint("tag_dispatch", expr))
            },
            Format::TokenDispatch(f) => {
                let mut pairs = Vec::with_capacity(f.rules.len());
                for (i, (_, content)) in f.rules.iter().enumerate() {
                    let id = self.visit(content)?;
                    pairs.push((f.resolved_trigger_token_ids[i], id));
                }
                let expr = self.add_token_tag_dispatch_rule(
                    pairs,
                    f.loop_after_dispatch,
                    f.resolved_exclude_token_ids.clone(),
                );
                Ok(self.builder.add_rule_with_hint("token_tag_dispatch", expr))
            },
        }
    }

    fn visit_triggered_tags(
        &mut self,
        f: &TriggeredTagsFormat,
    ) -> Result<i32, Ist> {
        let mut trigger_to_tag_ids: Vec<Vec<usize>> =
            vec![Vec::new(); f.triggers.len()];
        let mut tag_content_rule_ids = Vec::with_capacity(f.tags.len());

        for (it_tag, tag) in f.tags.iter().enumerate() {
            let TagBegin::Str(tag_begin) = &tag.begin else {
                return Err(Ist::invalid(
                    "Tags in triggered_tags must have a string begin, not a token format",
                ));
            };
            let mut matched: Option<usize> = None;
            for (it_trigger, trigger) in f.triggers.iter().enumerate() {
                if tag_begin.starts_with(trigger) {
                    if matched.is_some() {
                        return Err(Ist::invalid(
                            "One tag matches multiple triggers in a triggered tags format",
                        ));
                    }
                    matched = Some(it_trigger);
                }
            }
            let Some(matched) = matched else {
                return Err(Ist::invalid(
                    "One tag does not match any trigger in a triggered tags format",
                ));
            };
            trigger_to_tag_ids[matched].push(it_tag);
            tag_content_rule_ids.push(self.visit(&tag.content)?);
        }

        // Special case: at_least_one && stop_after_first.
        if f.at_least_one && f.stop_after_first {
            let mut choices = Vec::new();
            for (it_tag, tag) in f.tags.iter().enumerate() {
                let begin = self.build_begin_expr(tag);
                let r = self.builder.add_rule_ref(tag_content_rule_ids[it_tag]);
                let end = self.build_end_expr(tag);
                choices.push(self.builder.add_sequence(&[begin, r, end]));
            }
            let choice = self.builder.add_choices(&choices);
            return Ok(self
                .builder
                .add_rule_with_hint("triggered_tags", choice));
        }

        // Normal case.
        let mut tag_rule_pairs = Vec::new();
        for (it_trigger, trigger) in f.triggers.iter().enumerate() {
            let mut choices = Vec::new();
            for &tag_id in &trigger_to_tag_ids[it_trigger] {
                let tag = &f.tags[tag_id];
                let TagBegin::Str(tag_begin) = &tag.begin else {
                    unreachable!("checked above");
                };
                let begin =
                    self.builder.add_byte_string(&tag_begin[trigger.len()..]);
                let r = self.builder.add_rule_ref(tag_content_rule_ids[tag_id]);
                let end = self.build_end_expr(tag);
                choices.push(self.builder.add_sequence(&[begin, r, end]));
            }
            let choice = self.builder.add_choices(&choices);
            let sub_rule =
                self.builder.add_rule_with_hint("triggered_tags_group", choice);
            tag_rule_pairs.push((trigger.clone(), sub_rule));
        }

        let mut all_excludes = f.excludes.clone();
        all_excludes.extend(
            f.detected_end_strs.iter().filter(|s| !s.is_empty()).cloned(),
        );
        let mut rule_expr_id = self.add_tag_dispatch_rule(
            tag_rule_pairs,
            !f.stop_after_first,
            all_excludes,
        );

        if f.at_least_one {
            let mut first_choices = Vec::new();
            for (it_tag, tag) in f.tags.iter().enumerate() {
                let begin = self.build_begin_expr(tag);
                let r = self.builder.add_rule_ref(tag_content_rule_ids[it_tag]);
                let end = self.build_end_expr(tag);
                first_choices.push(self.builder.add_sequence(&[begin, r, end]));
            }
            let first_choice = self.builder.add_choices(&first_choices);
            let first_rule = self
                .builder
                .add_rule_with_hint("triggered_tags_first", first_choice);
            let dispatch_rule = self
                .builder
                .add_rule_with_hint("triggered_tags_sub", rule_expr_id);
            let ref_first = self.builder.add_rule_ref(first_rule);
            let ref_dispatch = self.builder.add_rule_ref(dispatch_rule);
            let seq = self.builder.add_sequence(&[ref_first, ref_dispatch]);
            rule_expr_id = self.builder.add_choices(&[seq]);
        }

        Ok(self.builder.add_rule_with_hint("triggered_tags", rule_expr_id))
    }

    fn visit_tags_with_separator(
        &mut self,
        f: &TagsWithSeparatorFormat,
    ) -> Result<i32, Ist> {
        let mut choice_ids = Vec::with_capacity(f.tags.len());
        for tag in &f.tags {
            let tag_rule_id = self.visit(&Format::Tag(tag.clone()))?;
            let tag_ref = self.builder.add_rule_ref(tag_rule_id);
            choice_ids.push(self.builder.add_sequence(&[tag_ref]));
        }
        let choice = self.builder.add_choices(&choice_ids);
        let all_tags_rule =
            self.builder.add_rule_with_hint("tags_with_separator_tags", choice);
        let all_tags_ref = self.builder.add_rule_ref(all_tags_rule);

        if f.stop_after_first {
            let body = if f.at_least_one {
                let seq = self.builder.add_sequence(&[all_tags_ref]);
                self.builder.add_choices(&[seq])
            } else {
                let seq = self.builder.add_sequence(&[all_tags_ref]);
                let empty = self.builder.add_empty_str();
                self.builder.add_choices(&[seq, empty])
            };
            return Ok(self
                .builder
                .add_rule_with_hint("tags_with_separator", body));
        }

        let sub_rule =
            self.builder.add_empty_rule_with_hint("tags_with_separator_sub");
        let end_str_seq = self.builder.add_empty_str();
        let mut sub_seq = Vec::new();
        if !f.separator.is_empty() {
            sub_seq.push(self.builder.add_byte_string(&f.separator));
        }
        sub_seq.push(all_tags_ref);
        let sub_self_ref = self.builder.add_rule_ref(sub_rule);
        sub_seq.push(sub_self_ref);
        let sub_seq_id = self.builder.add_sequence(&sub_seq);
        let sub_body = self.builder.add_choices(&[sub_seq_id, end_str_seq]);
        self.builder.update_rule_body(sub_rule, sub_body);

        let sub_ref = self.builder.add_rule_ref(sub_rule);
        let main_seq = self.builder.add_sequence(&[all_tags_ref, sub_ref]);
        let mut choices = vec![main_seq];
        if !f.at_least_one {
            choices.push(end_str_seq);
        }
        let body = self.builder.add_choices(&choices);
        Ok(self.builder.add_rule_with_hint("tags_with_separator", body))
    }

    fn visit_token_triggered_tags(
        &mut self,
        f: &TokenTriggeredTagsFormat,
    ) -> Result<i32, Ist> {
        let mut trigger_to_tag_ids: Vec<Vec<usize>> =
            vec![Vec::new(); f.trigger_tokens.len()];
        let mut tag_content_rule_ids = Vec::with_capacity(f.tags.len());

        for (it_tag, tag) in f.tags.iter().enumerate() {
            let TagBegin::Token(tf) = &tag.begin else {
                return Err(Ist::invalid(
                    "Tags in token_triggered_tags must have a token format begin, not a string",
                ));
            };
            let begin_token_id = tf.resolved_token_id;
            let mut matched: Option<usize> = None;
            for (it, &tid) in f.resolved_trigger_token_ids.iter().enumerate() {
                if tid == begin_token_id {
                    if matched.is_some() {
                        return Err(Ist::invalid(
                            "Tag matches multiple triggers",
                        ));
                    }
                    matched = Some(it);
                }
            }
            let Some(matched) = matched else {
                return Err(Ist::invalid("Tag does not match any trigger"));
            };
            trigger_to_tag_ids[matched].push(it_tag);
            tag_content_rule_ids.push(self.visit(&tag.content)?);
        }

        if f.at_least_one && f.stop_after_first {
            let mut choices = Vec::new();
            for (it_tag, tag) in f.tags.iter().enumerate() {
                let begin = self.build_begin_expr(tag);
                let r = self.builder.add_rule_ref(tag_content_rule_ids[it_tag]);
                let end = self.build_end_expr(tag);
                choices.push(self.builder.add_sequence(&[begin, r, end]));
            }
            let choice = self.builder.add_choices(&choices);
            return Ok(self
                .builder
                .add_rule_with_hint("token_triggered_tags", choice));
        }

        let mut trigger_rule_pairs = Vec::new();
        for (it, &trigger_id) in f.resolved_trigger_token_ids.iter().enumerate()
        {
            let mut choices = Vec::new();
            for &tag_id in &trigger_to_tag_ids[it] {
                let tag = &f.tags[tag_id];
                let r = self.builder.add_rule_ref(tag_content_rule_ids[tag_id]);
                let end = self.build_end_expr(tag);
                choices.push(self.builder.add_sequence(&[r, end]));
            }
            let choice = self.builder.add_choices(&choices);
            let sub_rule = self
                .builder
                .add_rule_with_hint("token_triggered_tags_group", choice);
            trigger_rule_pairs.push((trigger_id, sub_rule));
        }

        let mut all_excludes = f.resolved_exclude_token_ids.clone();
        all_excludes.extend_from_slice(&f.detected_end_token_ids);
        let mut rule_expr_id = self.add_token_tag_dispatch_rule(
            trigger_rule_pairs,
            !f.stop_after_first,
            all_excludes,
        );

        if f.at_least_one {
            let mut first_choices = Vec::new();
            for (it_tag, tag) in f.tags.iter().enumerate() {
                let begin = self.build_begin_expr(tag);
                let r = self.builder.add_rule_ref(tag_content_rule_ids[it_tag]);
                let end = self.build_end_expr(tag);
                first_choices.push(self.builder.add_sequence(&[begin, r, end]));
            }
            let first_choice = self.builder.add_choices(&first_choices);
            let first_rule = self
                .builder
                .add_rule_with_hint("token_triggered_tags_first", first_choice);
            let dispatch_rule = self
                .builder
                .add_rule_with_hint("token_triggered_tags_sub", rule_expr_id);
            let ref_first = self.builder.add_rule_ref(first_rule);
            let ref_dispatch = self.builder.add_rule_ref(dispatch_rule);
            let seq = self.builder.add_sequence(&[ref_first, ref_dispatch]);
            rule_expr_id = self.builder.add_choices(&[seq]);
        }

        Ok(self
            .builder
            .add_rule_with_hint("token_triggered_tags", rule_expr_id))
    }
}
