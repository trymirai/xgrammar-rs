//! The grammar data model: the flat-buffer BNF representation, its builder, and EBNF
//! printing. Ported from `cpp/grammar_impl.h`, `grammar_builder.*`, and `grammar_printer.cc`.
//!
//! One dedicated type per file; re-exported here.

mod grammar_expr_type;

pub use grammar_expr_type::{GrammarExprType, UnknownGrammarExprType};
