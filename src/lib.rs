#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("src/bridge.hxx");

        #[namespace = "xgrammar"]
        type TokenizerInfo;
        #[namespace = "xgrammar"]
        type GrammarCompiler;
        #[namespace = "xgrammar"]
        type CompiledGrammar;

        #[namespace = "xgrammar"]
        fn xg_make_tokenizer_info(
            encoded_vocab: &CxxVector<CxxString>,
            vocab_type: i32,
            add_prefix_space: bool,
        ) -> UniquePtr<TokenizerInfo>;

        #[namespace = "xgrammar"]
        fn xg_make_compiler(
            tokenizer: &TokenizerInfo,
            max_threads: i32,
            cache_enabled: bool,
            max_memory_bytes: i64,
        ) -> UniquePtr<GrammarCompiler>;

        #[namespace = "xgrammar"]
        fn xg_compile_builtin_json(compiler: Pin<&mut GrammarCompiler>) -> UniquePtr<CompiledGrammar>;
    }
}

pub use ffi::*;


