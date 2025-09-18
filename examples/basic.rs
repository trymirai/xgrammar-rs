use autocxx::WithinBox;

fn main() {
    // Build the built-in JSON grammar
    let grammar = xgrammar::Grammar::BuiltinJSONGrammar().within_box();

    // EBNF string
    let ebnf = grammar.ToString();
    let ebnf_str = ebnf
        .as_ref()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from("<null>"));
    println!("EBNF (prefix): {}", &ebnf_str[..ebnf_str.len().min(200)]);

    // Serialized JSON of the grammar
    let json = grammar.SerializeJSON();
    let json_str = json
        .as_ref()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from("<null>"));
    println!("Serialized JSON (prefix): {}", &json_str[..json_str.len().min(200)]);
}
