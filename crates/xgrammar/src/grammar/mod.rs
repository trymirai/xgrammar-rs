//! The grammar data model: the BNF abstract syntax tree (AST), its builder, and EBNF printing.
//!
//! ## Rules
//!
//! The BNF grammar AST consists of a set of rules. Each rule contains a name and a definition,
//! and corresponds to a production in the grammar. The definition of a rule is a
//! [`GrammarExpr`]. Each rule has a `rule_id` for reference.
//!
//! ## GrammarExprs
//!
//! [`GrammarExpr`] is the definition of a rule or part of the definition of a rule. It can
//! contain elements, empty string, reference to other [`GrammarExpr`]s, or reference to other
//! rules. Each [`GrammarExpr`] corresponds to a `grammar_expr_id` for reference.
//!
//! For example, in the following rule: `rule ::= ("a" "b") | "c"`, `("a" "b")`, `"c"`, and
//! `("a" "b") | "c"` are all [`GrammarExpr`]s.
//!
//! ### Types of GrammarExprs
//!
//! Every [`GrammarExpr`] is represented by a type as well as a variable-length array containing
//! its data. [`GrammarExprType`] has several kinds:
//!
//! - **Byte string**: a string of bytes (0–255). Supports UTF-8 strings.
//! - **Character class**: a range of characters (each character is a Unicode codepoint), e.g.
//! `[a-z]`, `[ac-z]`. Can be negated: `[^a-z]`, `[^ac-z]`. Only ASCII characters are allowed
//! inside `[]`, but this expression can accept or reject Unicode characters.
//! - **Character class star**: a star quantifier of a character class, e.g. `[a-z]*`, `[^a-z]*`.
//! - **EmptyStr**: an empty string, i.e. `""`.
//! - **Rule reference**: a reference to another rule.
//! - **Sequence**: a sequence of grammar expressions, e.g. `("a" "b")`. These expressions are
//! concatenated together.
//! - **Choices**: a choice of grammar expressions, e.g. `("a" "b") | "c"`. Each expression can
//! be matched.
//!
//! ### Storage of GrammarExprs
//!
//! Each type of [`GrammarExpr`] has a different data format; see [`GrammarExprType`] for the
//! layout of each kind.
//!
//! All [`GrammarExpr`]s are stored in CSR-matrix style: they are stored consecutively in one
//! vector (the data vector) and the starting position of each [`GrammarExpr`] is recorded in the
//! indptr vector.
//!
//! ### Character class star
//!
//! The character-class-star [`GrammarExpr`] supports elements like `[a-z]*` in the grammar. It
//! makes matching more efficient by avoiding recursion into rules when matching a sequence of
//! characters. It should be used like:
//!
//! ```text
//! rule1 ::= ((element1 element2 rule2 .) | .)
//! rule2 ::= character_class_star_grammar_expr(id_of_a_character_class_grammar_expr)
//! ```

mod character_class_element;
mod grammar;
mod grammar_builder;
mod grammar_expr;
mod grammar_expr_type;
mod printer;
mod rule;
mod serialization;
mod tag_dispatch;
mod token_tag_dispatch;

pub use character_class_element::CharacterClassElement;
pub use grammar::Grammar;
pub use grammar_builder::GrammarBuilder;
pub use grammar_expr::GrammarExpr;
pub use grammar_expr_type::{GrammarExprType, UnknownGrammarExprType};
pub use rule::{NO_EXPR, Rule};
pub use serialization::DeserializeError;
pub use tag_dispatch::TagDispatch;
pub use token_tag_dispatch::TokenTagDispatch;
