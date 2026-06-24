//! The grammar data model: the flat-buffer BNF representation, its builder, and EBNF
//! printing. Ported from `cpp/grammar_impl.h`, `grammar_builder.*`, and `grammar_printer.cc`.
//!
//! One dedicated type per file; re-exported here.

mod character_class_element;
mod grammar;
mod grammar_builder;
mod grammar_expr;
mod grammar_expr_type;
mod rule;

pub use character_class_element::CharacterClassElement;
pub use grammar::Grammar;
pub use grammar_builder::GrammarBuilder;
pub use grammar_expr::GrammarExpr;
pub use grammar_expr_type::{GrammarExprType, UnknownGrammarExprType};
pub use rule::{NO_EXPR, Rule};
