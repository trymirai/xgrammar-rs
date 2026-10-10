use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{
    backends::{Backend, replace_self_in_return},
    contexts::{
        ClassContext, EnumerationContext, ErrorContext, ImplementationContext, MethodMetadata, StructureContext,
    },
    types::{MethodFlavor, StructureFlavor},
};

pub struct Wasm;

impl Backend for Wasm {
    fn structure_attributes(context: &StructureContext) -> TokenStream {
        match context.flavor {
            StructureFlavor::Plain => quote! {
                #[cfg_attr(feature = "bindings-wasm", derive(tsify::Tsify, serde::Serialize, serde::Deserialize))]
                #[cfg_attr(feature = "bindings-wasm", tsify(into_wasm_abi, from_wasm_abi))]
            },
            StructureFlavor::Class => quote! {
                #[cfg_attr(feature = "bindings-wasm", wasm_bindgen::prelude::wasm_bindgen)]
            },
        }
    }

    fn class_attributes(_context: &ClassContext) -> TokenStream {
        quote! {
            #[cfg_attr(feature = "bindings-wasm", wasm_bindgen::prelude::wasm_bindgen)]
        }
    }

    fn enumeration_attributes(_context: &EnumerationContext) -> TokenStream {
        quote! {
            #[cfg_attr(feature = "bindings-wasm", derive(tsify::Tsify, serde::Serialize, serde::Deserialize))]
            #[cfg_attr(feature = "bindings-wasm", tsify(into_wasm_abi, from_wasm_abi))]
        }
    }

    fn implementation_attributes(_context: &ImplementationContext) -> TokenStream {
        quote! {
            #[cfg_attr(feature = "bindings-wasm", wasm_bindgen::prelude::wasm_bindgen)]
        }
    }

    fn method_companions(
        context: &ImplementationContext,
        metadata: &MethodMetadata,
    ) -> TokenStream {
        match metadata.flavor {
            MethodFlavor::Constructor => constructor_expansion(context, metadata),
            MethodFlavor::Factory => factory_expansion(context, metadata),
            MethodFlavor::FactoryWithCallback => {
                let error = syn::Error::new_spanned(
                    &metadata.method.sig,
                    "Bindings::export(Method(FactoryWithCallback)) is not supported by the wasm backend",
                )
                .to_compile_error();
                quote! {
                    #[cfg(feature = "bindings-wasm")]
                    #error
                }
            },
            _ => quote! {},
        }
    }

    fn error_companions(context: &ErrorContext) -> TokenStream {
        let type_name = &context.item.ident;
        quote! {
            #[cfg(feature = "bindings-wasm")]
            impl From<#type_name> for wasm_bindgen::JsValue {
                fn from(error: #type_name) -> Self {
                    wasm_bindgen::JsValue::from_str(&error.to_string())
                }
            }
        }
    }
}

fn constructor_expansion(
    context: &ImplementationContext,
    metadata: &MethodMetadata,
) -> TokenStream {
    let allows = metadata.allow_attributes();
    let self_type = &context.self_type;
    let method = &metadata.method;
    let inputs = &method.sig.inputs;
    let output = replace_self_in_return(&method.sig.output, self_type);
    let asyncness = &method.sig.asyncness;
    let body = &method.block;

    // Body is inlined because constructors are extracted from the source impl.
    quote! {
        #[cfg(feature = "bindings-wasm")]
        #[wasm_bindgen::prelude::wasm_bindgen]
        impl #self_type {
            #[wasm_bindgen(constructor)]
            #( #allows )*
            pub #asyncness fn new( #inputs ) #output {
                #body
            }
        }
    }
}

fn factory_expansion(
    context: &ImplementationContext,
    metadata: &MethodMetadata,
) -> TokenStream {
    let allows = metadata.allow_attributes();
    let self_type = &context.self_type;
    let method = &metadata.method;
    let method_ident = &method.sig.ident;
    let method_name = method_ident.to_string();
    let wrapper_ident = format_ident!("{}_bindings_wasm", method_ident);
    let inputs = &method.sig.inputs;
    let output = replace_self_in_return(&method.sig.output, self_type);
    let asyncness = &method.sig.asyncness;
    let arg_idents = metadata.arg_idents();
    let forward_await = asyncness.as_ref().map(|_| quote! { .await });

    // Forwards to the inherent factory that the Rust backend re-emits.
    quote! {
        #[cfg(feature = "bindings-wasm")]
        #[wasm_bindgen::prelude::wasm_bindgen]
        impl #self_type {
            #[wasm_bindgen(js_name = #method_name)]
            #( #allows )*
            pub #asyncness fn #wrapper_ident( #inputs ) #output {
                <#self_type>::#method_ident( #( #arg_idents ),* )#forward_await
            }
        }
    }
}
