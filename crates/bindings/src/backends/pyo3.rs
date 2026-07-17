use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use crate::{
    backends::Backend,
    contexts::{
        ClassContext, EnumerationContext, ErrorContext, ImplementationContext,
        MethodMetadata, StructureContext,
    },
    types::MethodFlavor,
};

pub struct Pyo3;

impl Backend for Pyo3 {
    fn structure_attributes(_context: &StructureContext) -> TokenStream {
        quote! {
            #[cfg_attr(feature = "bindings-pyo3", pyo3::pyclass(get_all, from_py_object))]
        }
    }

    fn structure_companions(context: &StructureContext) -> TokenStream {
        let registration_block = registration(&context.item.ident);
        let constructor_block = struct_constructor(context.item);
        quote! {
            #registration_block
            #constructor_block
        }
    }

    fn class_attributes(_context: &ClassContext) -> TokenStream {
        quote! {
            #[cfg_attr(feature = "bindings-pyo3", pyo3::pyclass(from_py_object))]
        }
    }

    fn class_companions(context: &ClassContext) -> TokenStream {
        registration(&context.item.ident)
    }

    fn enumeration_attributes(_context: &EnumerationContext) -> TokenStream {
        quote! {
            #[cfg_attr(feature = "bindings-pyo3", pyo3::pyclass(eq, from_py_object))]
        }
    }

    fn enumeration_companions(context: &EnumerationContext) -> TokenStream {
        registration(&context.item.ident)
    }

    fn implementation_companions(
        context: &ImplementationContext
    ) -> TokenStream {
        implementation_expansion(context)
    }

    fn method_attributes(metadata: &MethodMetadata) -> TokenStream {
        match metadata.flavor {
            MethodFlavor::Constructor => quote! {
                #[cfg_attr(feature = "bindings-pyo3", new)]
            },
            MethodFlavor::Factory => quote! {
                #[cfg_attr(feature = "bindings-pyo3", staticmethod)]
            },
            _ => quote! {},
        }
    }

    fn method_companions(
        context: &ImplementationContext,
        metadata: &MethodMetadata,
    ) -> TokenStream {
        match metadata.flavor {
            MethodFlavor::Factory => {
                factory_expansion(&context.self_type, metadata)
            },
            MethodFlavor::FactoryWithCallback => {
                factory_with_callback_expansion(&context.self_type, metadata)
            },
            MethodFlavor::Constructor => {
                constructor_expansion(&context.self_type, metadata)
            },
            _ => quote! {},
        }
    }

    fn error_companions(context: &ErrorContext) -> TokenStream {
        error_implementations(&context.item.ident)
    }
}

fn implementation_expansion(context: &ImplementationContext) -> TokenStream {
    let mut wrappers: Vec<TokenStream> = Vec::new();
    for metadata in context.all_methods() {
        if matches!(
            metadata.flavor,
            MethodFlavor::Factory
                | MethodFlavor::FactoryWithCallback
                | MethodFlavor::Constructor
        ) {
            continue;
        }
        wrappers.push(build_method_wrapper(metadata));
    }

    if let Some(metadata) = context.methods_of(MethodFlavor::StreamNext).next()
    {
        wrappers.push(build_stream_protocol(&metadata.method));
    }

    if wrappers.is_empty() {
        return quote! {};
    }

    let self_type = &context.self_type;
    quote! {
        #[cfg(feature = "bindings-pyo3")]
        const _: () = {
            #[allow(unused_imports)]
            use ::pyo3::prelude::*;
            #[pyo3::pymethods]
            impl #self_type {
                #( #wrappers )*
            }
        };
    }
}

fn build_stream_protocol(next_method: &syn::ImplItemFn) -> TokenStream {
    let next_ident = &next_method.sig.ident;
    quote! {
        pub fn __aiter__(slf: ::pyo3::PyRef<'_, Self>) -> ::pyo3::PyRef<'_, Self> {
            slf
        }

        pub fn __anext__<'py>(
            &self,
            py: ::pyo3::Python<'py>,
        ) -> ::pyo3::PyResult<::pyo3::Bound<'py, ::pyo3::PyAny>> {
            let __this = ::std::clone::Clone::clone(self);
            ::pyo3_async_runtimes::tokio::future_into_py(py, async move {
                match __this.#next_ident().await {
                    ::std::option::Option::Some(value) => ::std::result::Result::Ok(value),
                    ::std::option::Option::None => ::std::result::Result::Err(
                        ::pyo3::exceptions::PyStopAsyncIteration::new_err("end of stream"),
                    ),
                }
            })
        }

        pub fn iterator(slf: ::pyo3::PyRef<'_, Self>) -> ::pyo3::PyRef<'_, Self> {
            slf
        }
    }
}

fn build_method_wrapper(metadata: &MethodMetadata) -> TokenStream {
    let method = &metadata.method;
    let flavor = metadata.flavor;
    let original_ident = &method.sig.ident;
    let original_name = original_ident.to_string();
    let wrapper_ident = format_ident!("__pyo3_{}", original_ident);
    let asyncness = metadata.is_async();

    let pyo3_attribute = if asyncness {
        match flavor {
            MethodFlavor::Constructor => quote! { #[new] },
            MethodFlavor::Factory => quote! { #[staticmethod] },
            _ => quote! {},
        }
    } else {
        match flavor {
            MethodFlavor::Constructor => quote! { #[new] },
            MethodFlavor::Factory => quote! { #[staticmethod] },
            MethodFlavor::Getter => quote! { #[getter] },
            MethodFlavor::Setter => quote! { #[setter] },
            MethodFlavor::Plain
            | MethodFlavor::FactoryWithCallback
            | MethodFlavor::StreamNext => {
                quote! {}
            },
        }
    };

    let receiver = metadata.receiver();
    let typed_args = metadata.typed_args();
    let arg_idents = metadata.arg_idents();
    let typed_arg_tokens: Vec<TokenStream> = typed_args
        .iter()
        .map(|pat_type| {
            let pattern = &pat_type.pat;
            let owned_type = strip_outer_ref(&pat_type.ty);
            quote! { #pattern: #owned_type }
        })
        .collect();
    let arg_call_tokens: Vec<TokenStream> = typed_args
        .iter()
        .zip(arg_idents.iter())
        .map(|(pat_type, ident)| {
            if matches!(pat_type.ty.as_ref(), syn::Type::Reference(_)) {
                quote! { &#ident }
            } else {
                quote! { #ident }
            }
        })
        .collect();

    let receiver_tokens = match receiver {
        Some(receiver_arg) if receiver_arg.mutability.is_some() => {
            quote! { &mut self }
        },
        Some(_) => quote! { &self },
        None => quote! {},
    };
    let receiver_separator =
        if receiver.is_some() && !typed_arg_tokens.is_empty() {
            quote! { , }
        } else {
            quote! {}
        };

    let pyo3_name_attribute = match flavor {
        MethodFlavor::Constructor => quote! {},
        _ => quote! { #[pyo3(name = #original_name)] },
    };

    let call_target = if receiver.is_some() {
        quote! { self.#original_ident }
    } else {
        quote! { Self::#original_ident }
    };

    if asyncness {
        let py_argument = quote! { py: ::pyo3::Python<'py> };
        let inputs_with_py = if receiver.is_some() {
            if typed_arg_tokens.is_empty() {
                quote! { #receiver_tokens, #py_argument }
            } else {
                quote! { #receiver_tokens, #py_argument, #( #typed_arg_tokens ),* }
            }
        } else if typed_arg_tokens.is_empty() {
            quote! { #py_argument }
        } else {
            quote! { #py_argument, #( #typed_arg_tokens ),* }
        };
        let capture = if receiver.is_some() {
            quote! { let __this = ::std::clone::Clone::clone(self); }
        } else {
            quote! {}
        };
        let invocation = if receiver.is_some() {
            quote! { __this.#original_ident( #( #arg_call_tokens ),* ).await }
        } else {
            quote! { Self::#original_ident( #( #arg_call_tokens ),* ).await }
        };
        let body = if metadata.returns_result() {
            quote! {
                let __result = #invocation;
                __result.map_err(::std::convert::Into::into)
            }
        } else {
            quote! {
                let __value = #invocation;
                ::std::result::Result::<_, ::pyo3::PyErr>::Ok(__value)
            }
        };
        quote! {
            #pyo3_attribute
            #pyo3_name_attribute
            pub fn #wrapper_ident<'py>(
                #inputs_with_py
            ) -> ::pyo3::PyResult<::pyo3::Bound<'py, ::pyo3::PyAny>> {
                #capture
                ::pyo3_async_runtimes::tokio::future_into_py(py, async move {
                    #body
                })
            }
        }
    } else {
        let output = &method.sig.output;
        let inputs = if receiver.is_some() {
            quote! { #receiver_tokens #receiver_separator #( #typed_arg_tokens ),* }
        } else {
            quote! { #( #typed_arg_tokens ),* }
        };
        quote! {
            #pyo3_attribute
            #pyo3_name_attribute
            pub fn #wrapper_ident( #inputs ) #output {
                #call_target( #( #arg_call_tokens ),* )
            }
        }
    }
}

fn strip_outer_ref(ty: &syn::Type) -> TokenStream {
    if let syn::Type::Reference(reference) = ty {
        let inner = &reference.elem;
        quote! { #inner }
    } else {
        quote! { #ty }
    }
}

fn factory_expansion(
    self_type: &syn::Type,
    metadata: &MethodMetadata,
) -> TokenStream {
    factory_or_constructor_expansion(self_type, metadata, false)
}

fn constructor_expansion(
    self_type: &syn::Type,
    metadata: &MethodMetadata,
) -> TokenStream {
    factory_or_constructor_expansion(self_type, metadata, true)
}

fn factory_or_constructor_expansion(
    self_type: &syn::Type,
    metadata: &MethodMetadata,
    is_constructor: bool,
) -> TokenStream {
    let method = &metadata.method;
    let method_ident = &method.sig.ident;
    let method_name = method_ident.to_string();
    let wrapper_ident = format_ident!("{}_bindings_pyo3", method_ident);
    let output = &method.sig.output;
    let asyncness = metadata.is_async();
    let arg_idents = metadata.arg_idents();
    let body_block = &method.block;

    // pyo3 cannot extract a bare `&T` (reference to an exported class) as a pymethod
    // argument, so the wrapper takes every argument by value (`strip_outer_ref`). Factory
    // wrappers re-borrow when forwarding to the inherent method (which keeps the original
    // `&T` signature, re-emitted by the `Rust` backend); constructor wrappers run the
    // original body inline, so each former-reference argument is shadow-rebound to a
    // reference first.
    let typed_args = metadata.typed_args();
    let inputs: Vec<TokenStream> = typed_args
        .iter()
        .map(|pat_type| {
            let pattern = &pat_type.pat;
            let owned_type = strip_outer_ref(&pat_type.ty);
            quote! { #pattern: #owned_type }
        })
        .collect();
    let arg_call_tokens: Vec<TokenStream> = typed_args
        .iter()
        .zip(arg_idents.iter())
        .map(|(pat_type, ident)| {
            if matches!(pat_type.ty.as_ref(), syn::Type::Reference(_)) {
                quote! { &#ident }
            } else {
                quote! { #ident }
            }
        })
        .collect();
    let reborrow_shadows: Vec<TokenStream> = typed_args
        .iter()
        .zip(arg_idents.iter())
        .filter(|(pat_type, _)| {
            matches!(pat_type.ty.as_ref(), syn::Type::Reference(_))
        })
        .map(|(_, ident)| quote! { let #ident = &#ident; })
        .collect();
    let inputs = quote! { #( #inputs ),* };

    let pyo3_method_kind = if is_constructor {
        quote! { #[new] }
    } else {
        quote! {
            #[staticmethod]
            #[pyo3(name = #method_name)]
        }
    };
    let invocation = if is_constructor {
        quote! { #( #reborrow_shadows )* #body_block }
    } else {
        quote! {
            <#self_type>::#method_ident( #( #arg_call_tokens ),* )
        }
    };

    if asyncness {
        let body = if is_constructor {
            quote! { async move { #( #reborrow_shadows )* #body_block } }
        } else if metadata.returns_result() {
            quote! {
                let __result = <#self_type>::#method_ident( #( #arg_call_tokens ),* ).await;
                __result.map_err(::std::convert::Into::into)
            }
        } else {
            quote! {
                let __value = <#self_type>::#method_ident( #( #arg_call_tokens ),* ).await;
                ::std::result::Result::<_, ::pyo3::PyErr>::Ok(__value)
            }
        };
        quote! {
            #[cfg(feature = "bindings-pyo3")]
            const _: () = {
                #[allow(unused_imports)]
                use ::pyo3::prelude::*;
                #[pyo3::pymethods]
                impl #self_type {
                    #pyo3_method_kind
                    pub fn #wrapper_ident<'py>(
                        py: ::pyo3::Python<'py>,
                        #inputs
                    ) -> ::pyo3::PyResult<::pyo3::Bound<'py, ::pyo3::PyAny>> {
                        ::pyo3_async_runtimes::tokio::future_into_py(py, async move {
                            #body
                        })
                    }
                }
            };
        }
    } else {
        quote! {
            #[cfg(feature = "bindings-pyo3")]
            const _: () = {
                #[allow(unused_imports)]
                use ::pyo3::prelude::*;
                #[pyo3::pymethods]
                impl #self_type {
                    #pyo3_method_kind
                    pub fn #wrapper_ident( #inputs ) #output {
                        #invocation
                    }
                }
            };
        }
    }
}

fn factory_with_callback_expansion(
    self_type: &syn::Type,
    metadata: &MethodMetadata,
) -> TokenStream {
    let method = &metadata.method;
    let method_ident = &method.sig.ident;
    let method_name = method_ident.to_string();
    let body = &method.block;
    let wrapper_ident = format_ident!("{}_bindings_pyo3", method_ident);

    let callback_inputs = match metadata.callback_inputs() {
        Some(inputs) => inputs,
        None => {
            return syn::Error::new_spanned(
                &method.sig,
                "Bindings::export(Method(FactoryWithCallback)) requires a parameter of type \
                 `Box<dyn Fn(..) + Send + Sync>` (return type must be `()`)",
            )
            .to_compile_error();
        },
    };
    let synthetic_idents: Vec<Ident> = (0..callback_inputs.len())
        .map(|index| format_ident!("arg{index}"))
        .collect();
    let py_call_arguments = if synthetic_idents.is_empty() {
        quote! { () }
    } else {
        quote! { ( #( #synthetic_idents ),* , ) }
    };
    quote! {
        #[cfg(feature = "bindings-pyo3")]
        const _: () = {
            #[allow(unused_imports)]
            use ::pyo3::prelude::*;
            #[pyo3::pymethods]
            impl #self_type {
                #[staticmethod]
                #[pyo3(name = #method_name)]
                pub fn #wrapper_ident(
                    callback: ::pyo3::Py<::pyo3::PyAny>,
                ) -> ::pyo3::PyResult<Self> {
                    let callback: ::std::boxed::Box<
                        dyn ::std::ops::Fn( #( #callback_inputs ),* ) + ::std::marker::Send + ::std::marker::Sync,
                    > = ::std::boxed::Box::new(
                        move | #( #synthetic_idents: #callback_inputs ),* | {
                            ::pyo3::Python::attach(|py| {
                                let _ = callback.call1(py, #py_call_arguments);
                            });
                        },
                    );
                    ::std::result::Result::Ok(#body)
                }
            }
        };
    }
}

fn struct_constructor(item_struct: &syn::ItemStruct) -> TokenStream {
    let type_name = &item_struct.ident;
    let fields = match &item_struct.fields {
        syn::Fields::Named(named) => &named.named,
        _ => return quote! {},
    };
    let public_fields: Vec<&syn::Field> = fields
        .iter()
        .filter(|field| matches!(field.vis, syn::Visibility::Public(_)))
        .collect();
    if public_fields.is_empty() {
        return quote! {};
    }
    let parameters: Vec<TokenStream> = public_fields
        .iter()
        .map(|field| {
            let ident = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            quote! { #ident: #ty }
        })
        .collect();
    let assignments: Vec<TokenStream> = public_fields
        .iter()
        .map(|field| {
            let ident = field.ident.as_ref().unwrap();
            quote! { #ident }
        })
        .collect();
    quote! {
        #[cfg(feature = "bindings-pyo3")]
        const _: () = {
            #[allow(unused_imports)]
            use ::pyo3::prelude::*;
            #[pyo3::pymethods]
            impl #type_name {
                #[new]
                fn __pyo3_new(#( #parameters ),*) -> Self {
                    Self { #( #assignments ),* }
                }
            }
        };
    }
}

fn registration(type_name: &Ident) -> TokenStream {
    let type_name_string = type_name.to_string();
    quote! {
        #[cfg(feature = "bindings-pyo3")]
        ::inventory::submit! {
            ::bindings_types::PyClassRegistration {
                register: |module| {
                    use ::pyo3::types::{PyAnyMethods, PyModuleMethods};
                    module.add_class::<#type_name>()?;
                    let cls = module.as_any().getattr(#type_name_string)?;
                    let module_name: ::std::string::String =
                        module.as_any().getattr("__name__")?.extract()?;
                    let public_module = module_name
                        .rsplit_once('.')
                        .map(|(parent, _)| parent.to_owned())
                        .unwrap_or(module_name);
                    cls.setattr("__module__", public_module)?;
                    ::std::result::Result::Ok(())
                },
            }
        }
    }
}

fn error_implementations(type_name: &Ident) -> TokenStream {
    let from_py_message =
        format!("{} cannot be received from Python", type_name);
    quote! {
        #[cfg(feature = "bindings-pyo3")]
        impl From<#type_name> for ::pyo3::PyErr {
            fn from(error: #type_name) -> Self {
                ::pyo3::exceptions::PyRuntimeError::new_err(error.to_string())
            }
        }

        #[cfg(feature = "bindings-pyo3")]
        impl<'a, 'py> ::pyo3::FromPyObject<'a, 'py> for #type_name {
            type Error = ::pyo3::PyErr;
            fn extract(_obj: ::pyo3::Borrowed<'a, 'py, ::pyo3::PyAny>) -> ::std::result::Result<Self, Self::Error> {
                ::std::result::Result::Err(
                    ::pyo3::exceptions::PyTypeError::new_err(#from_py_message),
                )
            }
        }

        #[cfg(feature = "bindings-pyo3")]
        impl<'py> ::pyo3::IntoPyObject<'py> for #type_name {
            type Target = ::pyo3::types::PyString;
            type Output = ::pyo3::Bound<'py, ::pyo3::types::PyString>;
            type Error = ::std::convert::Infallible;

            fn into_pyobject(self, py: ::pyo3::Python<'py>) -> ::std::result::Result<Self::Output, Self::Error> {
                ::std::result::Result::Ok(::pyo3::types::PyString::new(py, &self.to_string()))
            }
        }

    }
}
