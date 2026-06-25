//! Parses a structural-tag JSON document into the [`Format`] IR — a port of
//! `StructuralTagParser` in `cpp/structural_tag.cc`.

use serde_json::{Map, Value};

use super::{
    structural_tag_error::StructuralTagError,
    structural_tag_format::{
        AnyTextFormat, AnyTokensFormat, ConstStringFormat, DispatchFormat,
        ExcludeTokenFormat, Format, GrammarFormat, IntOrString,
        JsonSchemaFormat, OptionalFormat, OrFormat, PlusFormat, RegexFormat,
        RepeatFormat, SequenceFormat, StarFormat, TagBegin, TagEnd, TagFormat,
        TagsWithSeparatorFormat, TokenDispatchFormat, TokenFormat,
        TokenTriggeredTagsFormat, TriggeredTagsFormat,
    },
};

type Ist = StructuralTagError;

fn err<T>(message: &str) -> Result<T, Ist> {
    Err(Ist::invalid(message))
}

/// Parses a structural-tag JSON string into its [`Format`].
pub(crate) fn parse_structural_tag(
    json: &str
) -> Result<Format, StructuralTagError> {
    let value: Value = serde_json::from_str(json)
        .map_err(|e| Ist::InvalidJson(format!("Failed to parse JSON: {e}")))?;
    parse_structural_tag_value(&value)
}

fn parse_structural_tag_value(value: &Value) -> Result<Format, Ist> {
    let Some(obj) = value.as_object() else {
        return err("Structural tag must be an object");
    };
    if let Some(ty) = obj.get("type") {
        if ty.as_str() != Some("structural_tag") {
            return err(
                "Structural tag's type must be a string \"structural_tag\"",
            );
        }
    }
    let Some(format) = obj.get("format") else {
        return err("Structural tag must have a format field");
    };
    parse_format(format)
}

fn parse_format(value: &Value) -> Result<Format, Ist> {
    let Some(obj) = value.as_object() else {
        return err("Format must be an object");
    };
    if let Some(ty) = obj.get("type") {
        let Some(ty) = ty.as_str() else {
            return err("Format's type must be a string");
        };
        return match ty {
            "const_string" => Ok(Format::ConstString(parse_const_string(obj)?)),
            "json_schema" => {
                Ok(Format::JsonSchema(parse_json_schema(obj, None)?))
            },
            "any_text" => Ok(Format::AnyText(parse_any_text(obj)?)),
            "sequence" => Ok(Format::Sequence(parse_sequence(obj)?)),
            "or" => Ok(Format::Or(parse_or(obj)?)),
            "tag" => Ok(Format::Tag(parse_tag(obj)?)),
            "triggered_tags" => {
                Ok(Format::TriggeredTags(parse_triggered_tags(obj)?))
            },
            "tags_with_separator" => {
                Ok(Format::TagsWithSeparator(parse_tags_with_separator(obj)?))
            },
            "optional" => Ok(Format::Optional(OptionalFormat {
                content: Box::new(parse_content(obj, "Optional")?),
            })),
            "plus" => Ok(Format::Plus(PlusFormat {
                content: Box::new(parse_content(obj, "Plus")?),
            })),
            "star" => Ok(Format::Star(StarFormat {
                content: Box::new(parse_content(obj, "Star")?),
            })),
            "repeat" => Ok(Format::Repeat(parse_repeat(obj)?)),
            "qwen_xml_parameter" => Ok(Format::JsonSchema(parse_json_schema(
                obj,
                Some("qwen_xml"),
            )?)),
            "grammar" => Ok(Format::Grammar(parse_grammar(obj)?)),
            "regex" => Ok(Format::Regex(parse_regex(obj)?)),
            "token" => Ok(Format::Token(parse_token(obj)?)),
            "exclude_token" => {
                Ok(Format::ExcludeToken(parse_exclude_token(obj)?))
            },
            "any_tokens" => Ok(Format::AnyTokens(parse_any_tokens(obj)?)),
            "token_triggered_tags" => {
                Ok(Format::TokenTriggeredTags(parse_token_triggered_tags(obj)?))
            },
            "dispatch" => Ok(Format::Dispatch(parse_dispatch(obj)?)),
            "token_dispatch" => {
                Ok(Format::TokenDispatch(parse_token_dispatch(obj)?))
            },
            other => Err(Ist::invalid(format!(
                "Format type not recognized: {other}"
            ))),
        };
    }

    // No type: try each format, tag first.
    if let Ok(f) = parse_tag(obj) {
        return Ok(Format::Tag(f));
    }
    if let Ok(f) = parse_const_string(obj) {
        return Ok(Format::ConstString(f));
    }
    if let Ok(f) = parse_json_schema(obj, None) {
        return Ok(Format::JsonSchema(f));
    }
    if let Ok(f) = parse_any_text(obj) {
        return Ok(Format::AnyText(f));
    }
    if let Ok(f) = parse_sequence(obj) {
        return Ok(Format::Sequence(f));
    }
    if let Ok(f) = parse_or(obj) {
        return Ok(Format::Or(f));
    }
    if let Ok(f) = parse_triggered_tags(obj) {
        return Ok(Format::TriggeredTags(f));
    }
    if let Ok(f) = parse_tags_with_separator(obj) {
        return Ok(Format::TagsWithSeparator(f));
    }
    if let Ok(content) = parse_content(obj, "Optional") {
        return Ok(Format::Optional(OptionalFormat {
            content: Box::new(content),
        }));
    }
    if let Ok(f) = parse_repeat(obj) {
        return Ok(Format::Repeat(f));
    }
    if let Ok(f) = parse_dispatch(obj) {
        return Ok(Format::Dispatch(f));
    }
    if let Ok(f) = parse_token_dispatch(obj) {
        return Ok(Format::TokenDispatch(f));
    }
    Err(Ist::invalid(format!("Invalid format: {value}")))
}

fn parse_const_string(
    obj: &Map<String, Value>
) -> Result<ConstStringFormat, Ist> {
    match obj.get("value").and_then(Value::as_str) {
        Some(value) => Ok(ConstStringFormat {
            value: value.to_owned(),
        }),
        None => err("ConstString format must have a value field with a string"),
    }
}

fn parse_json_schema(
    obj: &Map<String, Value>,
    style_override: Option<&str>,
) -> Result<JsonSchemaFormat, Ist> {
    let Some(js) = obj.get("json_schema") else {
        return err(
            "JSON schema format must have a json_schema field with a object or boolean value",
        );
    };
    if !js.is_object() && !js.is_boolean() {
        return err(
            "JSON schema format must have a json_schema field with a object or boolean value",
        );
    }
    let style = if let Some(s) = style_override {
        s.to_owned()
    } else if let Some(s) = obj.get("style").and_then(Value::as_str) {
        if !["json", "qwen_xml", "minimax_xml", "deepseek_xml", "glm_xml"]
            .contains(&s)
        {
            return err(
                "style must be \"json\", \"qwen_xml\", \"minimax_xml\", \"deepseek_xml\", or \"glm_xml\"",
            );
        }
        s.to_owned()
    } else {
        "json".to_owned()
    };
    Ok(JsonSchemaFormat {
        json_schema: js.to_string(),
        style,
    })
}

fn parse_any_text(obj: &Map<String, Value>) -> Result<AnyTextFormat, Ist> {
    let Some(excludes) = obj.get("excludes") else {
        if !obj.contains_key("type") {
            return err(
                "Any text format should not have any fields other than type",
            );
        }
        return Ok(AnyTextFormat::default());
    };
    let Some(arr) = excludes.as_array() else {
        return err("AnyText format's excluded_strs field must be an array");
    };
    let mut out = Vec::with_capacity(arr.len());
    for e in arr {
        let Some(s) = e.as_str() else {
            return err(
                "AnyText format's excluded_strs array must contain strings",
            );
        };
        out.push(s.to_owned());
    }
    Ok(AnyTextFormat {
        excludes: out,
        detected_end_strs: Vec::new(),
    })
}

fn parse_grammar(obj: &Map<String, Value>) -> Result<GrammarFormat, Ist> {
    match obj.get("grammar").and_then(Value::as_str) {
        Some(g) if !g.is_empty() => Ok(GrammarFormat {
            grammar: g.to_owned(),
        }),
        _ => err(
            "Grammar format must have a grammar field with a non-empty string",
        ),
    }
}

fn parse_regex(obj: &Map<String, Value>) -> Result<RegexFormat, Ist> {
    match obj.get("pattern").and_then(Value::as_str) {
        Some(p) if !p.is_empty() => Ok(RegexFormat {
            pattern: p.to_owned(),
        }),
        _ => err(
            "Regex format must have a pattern field with a non-empty string",
        ),
    }
}

fn parse_elements(
    obj: &Map<String, Value>,
    kind: &str,
) -> Result<Vec<Format>, Ist> {
    let Some(arr) = obj.get("elements").and_then(Value::as_array) else {
        return Err(Ist::invalid(format!(
            "{kind} format must have an elements field with an array"
        )));
    };
    let mut out = Vec::with_capacity(arr.len());
    for e in arr {
        out.push(parse_format(e)?);
    }
    if out.is_empty() {
        return Err(Ist::invalid(format!(
            "{kind} format must have at least one element"
        )));
    }
    Ok(out)
}

fn parse_sequence(obj: &Map<String, Value>) -> Result<SequenceFormat, Ist> {
    Ok(SequenceFormat {
        elements: parse_elements(obj, "Sequence")?,
        is_unlimited: false,
    })
}

fn parse_or(obj: &Map<String, Value>) -> Result<OrFormat, Ist> {
    Ok(OrFormat {
        elements: parse_elements(obj, "Or")?,
        is_unlimited: false,
    })
}

fn parse_content(
    obj: &Map<String, Value>,
    kind: &str,
) -> Result<Format, Ist> {
    let Some(content) = obj.get("content") else {
        return Err(Ist::invalid(format!(
            "{kind} format must have a content field"
        )));
    };
    parse_format(content)
}

fn parse_tag_value(value: &Value) -> Result<TagFormat, Ist> {
    let Some(obj) = value.as_object() else {
        return err("Tag format must be an object");
    };
    if let Some(ty) = obj.get("type") {
        if ty.as_str() != Some("tag") {
            return err("Tag format's type must be a string \"tag\"");
        }
    }
    parse_tag(obj)
}

fn parse_tag(obj: &Map<String, Value>) -> Result<TagFormat, Ist> {
    let Some(begin_val) = obj.get("begin") else {
        return err("Tag format's begin field must be a string");
    };
    let begin = if let Some(s) = begin_val.as_str() {
        TagBegin::Str(s.to_owned())
    } else if let Some(o) = begin_val.as_object() {
        TagBegin::Token(parse_token(o)?)
    } else {
        return err("Tag format's begin field must be a string");
    };

    let Some(content) = obj.get("content") else {
        return err("Tag format must have a content field");
    };
    let content = parse_format(content)?;

    let Some(end_val) = obj.get("end") else {
        return err("Tag format must have an end field");
    };
    let end = if let Some(s) = end_val.as_str() {
        TagEnd::Strings(vec![s.to_owned()])
    } else if let Some(arr) = end_val.as_array() {
        if arr.is_empty() {
            return err("Tag format's end array cannot be empty");
        }
        let mut strings = Vec::with_capacity(arr.len());
        for item in arr {
            let Some(s) = item.as_str() else {
                return err("Tag format's end array must contain only strings");
            };
            strings.push(s.to_owned());
        }
        TagEnd::Strings(strings)
    } else if let Some(o) = end_val.as_object() {
        TagEnd::Token(parse_token(o)?)
    } else {
        return err(
            "Tag format's end field must be a string or array of strings",
        );
    };

    Ok(TagFormat {
        begin,
        content: Box::new(content),
        end,
    })
}

fn parse_bool_field(
    obj: &Map<String, Value>,
    key: &str,
) -> Result<bool, Ist> {
    match obj.get(key) {
        None => Ok(false),
        Some(v) => v
            .as_bool()
            .ok_or_else(|| Ist::invalid(format!("{key} must be a boolean"))),
    }
}

fn parse_tags(
    obj: &Map<String, Value>,
    missing_msg: &str,
    empty_msg: &str,
) -> Result<Vec<TagFormat>, Ist> {
    let Some(arr) = obj.get("tags").and_then(Value::as_array) else {
        return Err(Ist::invalid(missing_msg));
    };
    let mut tags = Vec::with_capacity(arr.len());
    for t in arr {
        tags.push(parse_tag_value(t)?);
    }
    if tags.is_empty() {
        return Err(Ist::invalid(empty_msg));
    }
    Ok(tags)
}

fn parse_triggered_tags(
    obj: &Map<String, Value>
) -> Result<TriggeredTagsFormat, Ist> {
    let Some(triggers_arr) = obj.get("triggers").and_then(Value::as_array)
    else {
        return err(
            "Triggered tags format must have a triggers field with an array",
        );
    };
    let mut triggers = Vec::with_capacity(triggers_arr.len());
    for t in triggers_arr {
        match t.as_str() {
            Some(s) if !s.is_empty() => triggers.push(s.to_owned()),
            _ => {
                return err(
                    "Triggered tags format's triggers must be non-empty strings",
                );
            },
        }
    }
    if triggers.is_empty() {
        return err("Triggered tags format's triggers must be non-empty");
    }
    let tags = parse_tags(
        obj,
        "Triggered tags format must have a tags field with an array",
        "Triggered tags format's tags must be non-empty",
    )?;
    let mut excludes = Vec::new();
    if let Some(ex) = obj.get("excludes") {
        let Some(arr) = ex.as_array() else {
            return err(
                "Triggered tags format should have a excludes field with an array",
            );
        };
        for e in arr {
            match e.as_str() {
                Some(s) if !s.is_empty() => excludes.push(s.to_owned()),
                _ => {
                    return err(
                        "Triggered tags format's excluded_strs must be non-empty strings",
                    );
                },
            }
        }
    }
    Ok(TriggeredTagsFormat {
        triggers,
        tags,
        excludes,
        at_least_one: parse_bool_field(obj, "at_least_one")?,
        stop_after_first: parse_bool_field(obj, "stop_after_first")?,
        detected_end_strs: Vec::new(),
    })
}

fn parse_tags_with_separator(
    obj: &Map<String, Value>
) -> Result<TagsWithSeparatorFormat, Ist> {
    let tags = parse_tags(
        obj,
        "Tags with separator format must have a tags field with an array",
        "Tags with separator format's tags must be non-empty",
    )?;
    let Some(separator) = obj.get("separator").and_then(Value::as_str) else {
        return err(
            "Tags with separator format's separator field must be a string",
        );
    };
    Ok(TagsWithSeparatorFormat {
        tags,
        separator: separator.to_owned(),
        at_least_one: parse_bool_field(obj, "at_least_one")?,
        stop_after_first: parse_bool_field(obj, "stop_after_first")?,
    })
}

fn parse_token(obj: &Map<String, Value>) -> Result<TokenFormat, Ist> {
    let Some(token) = obj.get("token") else {
        return err("TokenFormat must have a token field");
    };
    if token.is_string() {
        let s = token.as_str().unwrap();
        if s.is_empty() {
            return err("Token string must be non-empty");
        }
        Ok(TokenFormat::new(IntOrString::Str(s.to_owned())))
    } else if let Some(d) = token.as_f64() {
        if d != (d as i32) as f64 {
            return err("Token ID must be an integer");
        }
        let id = d as i32;
        if id < 0 {
            return err("Token ID must be non-negative");
        }
        Ok(TokenFormat::new(IntOrString::Int(id)))
    } else {
        err("TokenFormat's token must be an integer or string")
    }
}

fn parse_int_or_string_array(
    val: &Value,
    field: &str,
) -> Result<Vec<IntOrString>, Ist> {
    let Some(arr) = val.as_array() else {
        return Err(Ist::invalid(format!("{field} must be an array")));
    };
    let mut out = Vec::with_capacity(arr.len());
    for v in arr {
        if v.is_string() {
            let s = v.as_str().unwrap();
            if s.is_empty() {
                return Err(Ist::invalid(format!(
                    "{field} string elements must be non-empty"
                )));
            }
            out.push(IntOrString::Str(s.to_owned()));
        } else if let Some(d) = v.as_f64() {
            if d != (d as i32) as f64 {
                return Err(Ist::invalid(format!(
                    "{field} elements must be integers, not floats"
                )));
            }
            let id = d as i32;
            if id < 0 {
                return Err(Ist::invalid(format!(
                    "{field} elements must be non-negative integers or strings"
                )));
            }
            out.push(IntOrString::Int(id));
        } else {
            return Err(Ist::invalid(format!(
                "{field} elements must be integers or strings"
            )));
        }
    }
    Ok(out)
}

fn parse_exclude_token(
    obj: &Map<String, Value>
) -> Result<ExcludeTokenFormat, Ist> {
    let exclude_tokens = match obj.get("exclude_tokens") {
        Some(v) => parse_int_or_string_array(v, "exclude_tokens")?,
        None => Vec::new(),
    };
    Ok(ExcludeTokenFormat {
        exclude_tokens,
        ..ExcludeTokenFormat::default()
    })
}

fn parse_any_tokens(obj: &Map<String, Value>) -> Result<AnyTokensFormat, Ist> {
    let exclude_tokens = match obj.get("exclude_tokens") {
        Some(v) => parse_int_or_string_array(v, "exclude_tokens")?,
        None => Vec::new(),
    };
    Ok(AnyTokensFormat {
        exclude_tokens,
        ..AnyTokensFormat::default()
    })
}

fn parse_token_triggered_tags(
    obj: &Map<String, Value>
) -> Result<TokenTriggeredTagsFormat, Ist> {
    let Some(triggers_val) = obj.get("trigger_tokens") else {
        return err(
            "TokenTriggeredTagsFormat must have a trigger_tokens field",
        );
    };
    let trigger_tokens =
        parse_int_or_string_array(triggers_val, "trigger_tokens")?;
    if trigger_tokens.is_empty() {
        return err("trigger_tokens must be non-empty");
    }
    let tags = parse_tags(
        obj,
        "TokenTriggeredTagsFormat must have a tags field with an array",
        "TokenTriggeredTagsFormat tags must be non-empty",
    )?;
    let exclude_tokens = match obj.get("exclude_tokens") {
        Some(v) => parse_int_or_string_array(v, "exclude_tokens")?,
        None => Vec::new(),
    };
    Ok(TokenTriggeredTagsFormat {
        trigger_tokens,
        tags,
        exclude_tokens,
        at_least_one: parse_bool_field(obj, "at_least_one")?,
        stop_after_first: parse_bool_field(obj, "stop_after_first")?,
        resolved_trigger_token_ids: Vec::new(),
        resolved_exclude_token_ids: Vec::new(),
        detected_end_token_ids: Vec::new(),
    })
}

fn parse_dispatch(obj: &Map<String, Value>) -> Result<DispatchFormat, Ist> {
    let Some(rules_arr) = obj.get("rules").and_then(Value::as_array) else {
        return err("TagDispatch format must have a rules field with an array");
    };
    if rules_arr.is_empty() {
        return err("TagDispatch format rules must be non-empty");
    }
    let mut rules = Vec::with_capacity(rules_arr.len());
    for item in rules_arr {
        let Some(pair) = item.as_array() else {
            return err("TagDispatch pair must be a 2-element array");
        };
        if pair.len() != 2 {
            return err("TagDispatch pair must be a 2-element array");
        }
        let Some(trigger) = pair[0].as_str() else {
            return err("TagDispatch pair first element must be a string");
        };
        let content = parse_format(&pair[1])?;
        rules.push((trigger.to_owned(), Box::new(content)));
    }
    let mut excludes = Vec::new();
    if let Some(ex) = obj.get("excludes") {
        let Some(arr) = ex.as_array() else {
            return err("excludes must be an array");
        };
        for e in arr {
            match e.as_str() {
                Some(s) if !s.is_empty() => excludes.push(s.to_owned()),
                _ => return err("excludes must contain non-empty strings"),
            }
        }
    }
    Ok(DispatchFormat {
        rules,
        loop_after_dispatch: parse_bool_field(obj, "loop")?,
        excludes,
    })
}

fn parse_token_dispatch(
    obj: &Map<String, Value>
) -> Result<TokenDispatchFormat, Ist> {
    let Some(rules_arr) = obj.get("rules").and_then(Value::as_array) else {
        return err(
            "TokenTagDispatch format must have a rules field with an array",
        );
    };
    if rules_arr.is_empty() {
        return err("TokenTagDispatch format rules must be non-empty");
    }
    let mut rules = Vec::with_capacity(rules_arr.len());
    for item in rules_arr {
        let Some(pair) = item.as_array() else {
            return err("TokenTagDispatch pair must be a 2-element array");
        };
        if pair.len() != 2 {
            return err("TokenTagDispatch pair must be a 2-element array");
        }
        let trigger = if pair[0].is_string() {
            IntOrString::Str(pair[0].as_str().unwrap().to_owned())
        } else if let Some(d) = pair[0].as_f64() {
            if d != (d as i32) as f64 {
                return err("Token ID must be an integer");
            }
            IntOrString::Int(d as i32)
        } else {
            return err(
                "TokenTagDispatch pair first element must be an integer or string",
            );
        };
        let content = parse_format(&pair[1])?;
        rules.push((trigger, Box::new(content)));
    }
    let exclude_tokens = match obj.get("exclude_tokens") {
        Some(v) => parse_int_or_string_array(v, "exclude_tokens")?,
        None => Vec::new(),
    };
    Ok(TokenDispatchFormat {
        rules,
        loop_after_dispatch: parse_bool_field(obj, "loop")?,
        exclude_tokens,
        resolved_trigger_token_ids: Vec::new(),
        resolved_exclude_token_ids: Vec::new(),
    })
}

fn parse_repeat(obj: &Map<String, Value>) -> Result<RepeatFormat, Ist> {
    let Some(min) = obj.get("min").and_then(Value::as_f64) else {
        return err("Repeat format must have a min field (number)");
    };
    let Some(max) = obj.get("max").and_then(Value::as_f64) else {
        return err("Repeat format must have a max field (number)");
    };
    let min = min as i64;
    let mut max = max as i64;
    if max >= 0 && min > max {
        return err("Repeat min must be <= max");
    }
    if min < 0 {
        return err("Repeat min must be >= 0");
    }
    if max < -1 {
        return err("Repeat max must be -1 (unbounded) or >= 0");
    }
    if max > i64::from(i32::MAX) {
        max = -1;
    }
    if min > i64::from(i32::MAX) {
        return Err(Ist::invalid(format!(
            "Repeat min is too large, must be <= {}",
            i32::MAX
        )));
    }
    let content = parse_content(obj, "Repeat")?;
    Ok(RepeatFormat {
        min: min as i32,
        max: max as i32,
        content: Box::new(content),
    })
}
