use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    backends::Backend,
    contexts::{ImplementationContext, MethodMetadata},
    types::MethodFlavor,
};

pub struct Rust;

impl Backend for Rust {
    fn method_companions(
        context: &ImplementationContext,
        metadata: &MethodMetadata,
    ) -> TokenStream {
        match metadata.flavor {
            // Factories are extracted from the annotated impl; re-emit the inherent
            // method so other backends can forward to it. Constructors keep their
            // body only in backend-specific companions (named `new` for UniFFI).
            MethodFlavor::Factory => {
                let self_type = &context.self_type;
                let method = &metadata.method;
                quote! {
                    impl #self_type {
                        #method
                    }
                }
            },
            _ => quote! {},
        }
    }
}
