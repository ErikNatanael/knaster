extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::quote;
use syn::{DeriveInput, Expr, Ident, ImplItem, ItemImpl, Lit, parse_macro_input};

#[proc_macro_derive(KnasterIntegerParameter)]
pub fn knaster_integer_parameter(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    let ident = input.ident;

    let len = match input.data {
        syn::Data::Enum(enum_item) => enum_item.variants.len(),
        _ => panic!("KnasterIntegerParameter only works on Enums"),
    };

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
    impl From<PInteger> for #ident {
        fn from(value: PInteger) -> Self {
            <Self as num_traits::FromPrimitive>::from_usize(value.0).unwrap_or( #ident ::default())
        }
    }
    impl From< #ident > for PInteger {
        fn from(value: #ident) -> Self {
            PInteger(value as usize)
        }
    }
    impl PIntegerConvertible for #ident {
        fn pinteger_range() -> (PInteger, PInteger) {
            (
                PInteger(0),
                PInteger(#len),
            )
        }
            #[cfg(any(feature="std", feature="alloc"))]
        fn pinteger_descriptions(v: PInteger) -> std::string::String {
            format!("{:?}", #ident ::from(v))
        }
    }
            };

    // Hand the output tokens back to the compiler
    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn ugen(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemImpl);
    let crate_ident = get_knaster_crate_name();
    // Remove the ugen attribute from the impl block
    input
        .attrs
        .retain(|attr| !attr.path().segments.iter().any(|seg| seg.ident == "ugen"));
    let impl_block_generics = &input.generics;
    let struct_name = &input.self_ty;
    // optional
    let mut init_fn = None;
    // required
    let mut process_fn = None;
    // optional
    let mut process_block_fn = None;
    let mut param_fns = Vec::new();

    for item in &input.items {
        if let ImplItem::Fn(method) = item {
            match method.sig.ident.to_string().as_str() {
                "init" => {
                    init_fn = Some(method);
                }
                "process" => {
                    process_fn = Some(method);
                }
                "process_block" => {
                    process_block_fn = Some(method);
                }
                _ => (),
            }
            for attr in &method.attrs {
                if attr.path().is_ident("init") {
                    init_fn = Some(method);
                } else if attr.path().is_ident("process") {
                    process_fn = Some(method);
                } else if attr.path().is_ident("process_block") {
                    process_block_fn = Some(method);
                } else if attr.path().is_ident("param") {
                    param_fns.push(method);
                }
            }
        }
    }

    let init_impl = match init_fn {
        Some(init_fn) => quote! { #init_fn },
        None => quote! { fn init(&mut self, _: u32, _: usize) {} },
    };

    let mut num_input_channels = 0;
    let mut num_output_channels = 0;

    let process_impl = match process_fn {
        Some(process_fn) => {
            match &process_fn.sig.output {
                syn::ReturnType::Default => num_output_channels = 0,
                syn::ReturnType::Type(_rarrow, return_ty) => match &**return_ty {
                    syn::Type::Array(array) => {
                        num_output_channels = if let Expr::Lit(el) = &array.len {
                            if let Lit::Int(i) = &el.lit {
                                i.base10_parse().unwrap()
                            } else {
                                panic!(
                                    "process function must return an array of samples with a literal length"
                                )
                            }
                        } else {
                            panic!(
                                "process function must return an array of samples with a literal length"
                            )
                        };
                    }
                    _ => {
                        panic!("process function must return an array of samples")
                    }
                },
            }
            let mut process_args = Vec::new();

            let process_fn_name = &process_fn.sig.ident;
            for input in &process_fn.sig.inputs {
                if let syn::FnArg::Typed(pat_type) = input {
                    match &*pat_type.ty {
                        syn::Type::Reference(ref_type) => match &*ref_type.elem {
                            syn::Type::Path(path) => {
                                if path.path.segments.iter().any(|seg| seg.ident == "AudioCtx") {
                                    process_args.push(quote! { _ctx.into(), });
                                } else if path
                                    .path
                                    .segments
                                    .iter()
                                    .any(|seg| seg.ident == "UGenFlags")
                                {
                                    process_args.push(quote! { _flags.into(), });
                                } else {
                                    panic!("unknown argument found in process function");
                                };
                            }
                            _ => {
                                panic!("unknown argument found in process function");
                            }
                        },

                        syn::Type::Array(array) => {
                            num_input_channels = if let Expr::Lit(el) = &array.len {
                                if let Lit::Int(i) = &el.lit {
                                    i.base10_parse().unwrap()
                                } else {
                                    panic!("process function input array must use a literal length")
                                }
                            } else {
                                panic!("process function input array must use a literal length")
                            };
                            process_args.push(quote! { _input_array, });
                        }
                        _ => {
                            panic!("unknown argument found in process function");
                        }
                    }
                }
            }
            let input_array_elements = (0..num_input_channels)
                .map(|i: usize| quote! { _input[#i], })
                .collect::<Vec<_>>();

            quote! {
                fn process(
                    &mut self,
                    _ctx: &mut #crate_ident::AudioCtx,
                    _flags: &mut #crate_ident::UGenFlags,
                    _input: #crate_ident::Frame<Self::Sample, Self::Inputs>,
                ) -> #crate_ident::Frame<Self::Sample, Self::Outputs> {
                        let _input_array = [ #(#input_array_elements)* ];
                        Self:: #process_fn_name (self,  #(#process_args)* ).into()
                }
            }
        }
        None => quote! {
            fn process(
                &mut self,
                _ctx: &mut #crate_ident::AudioCtx,
                _flags: &mut #crate_ident::UGenFlags,
                _input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
            ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
                todo!()
            }
        },
    };

    // let process_block_impl = match process_block_fn {
    //     Some(process_fn) => {
    //         let mut process_args = Vec::new();
    //
    //         let process_fn_name = &process_fn.sig.ident;
    //         for input in &process_fn.sig.inputs {
    //             if let syn::FnArg::Typed(pat_type) = input {
    //                 match &*pat_type.ty {
    //                     syn::Type::Reference(ref_type) => match &*ref_type.elem {
    //                         syn::Type::Path(path) => {
    //                             if path.path.segments.iter().any(|seg| seg.ident == "AudioCtx") {
    //                                 process_args.push(quote! { _ctx.into(), });
    //                             } else if path
    //                                 .path
    //                                 .segments
    //                                 .iter()
    //                                 .any(|seg| seg.ident == "UGenFlags")
    //                             {
    //                                 process_args.push(quote! { _flags.into(), });
    //                             } else {
    //                                 panic!("unknown argument found in process function");
    //                             };
    //                         }
    //                         _ => {
    //                             panic!("unknown argument found in process function");
    //                         }
    //                     },
    //
    //                     // TODO: &[F] is input, &mut [F] is output
    //                     syn::Type::Array(array) => {
    //                         let num_input_channels = if let Expr::Lit(el) = &array.len {
    //                             if let Lit::Int(i) = &el.lit {
    //                                 i.base10_parse().unwrap()
    //                             } else {
    //                                 panic!("process function input array must use a literal length")
    //                             }
    //                         } else {
    //                             panic!("process function input array must use a literal length")
    //                         };
    //                         process_args.push(quote! { _input_array, });
    //                     }
    //                     _ => {
    //                         panic!("unknown argument found in process function");
    //                     }
    //                 }
    //             }
    //         }
    //         let input_array_elements = (0..num_input_channels)
    //             .map(|i: usize| quote! { _input[#i], })
    //             .collect::<Vec<_>>();
    //
    //         quote! {
    //             fn process(
    //                 &mut self,
    //                 _ctx: &mut #crate_ident::AudioCtx,
    //                 _flags: &mut #crate_ident::UGenFlags,
    //                 _input: #crate_ident::Frame<Self::Sample, Self::Inputs>,
    //             ) -> #crate_ident::Frame<Self::Sample, Self::Outputs> {
    //                     let _input_array = [ #(#input_array_elements)* ];
    //                     Self:: #process_fn_name (self,  #(#process_args)* ).into()
    //             }
    //         }
    //     }
    //     None => quote! {
    //         fn process(
    //             &mut self,
    //             _ctx: &mut #crate_ident::AudioCtx,
    //             _flags: &mut #crate_ident::UGenFlags,
    //             _input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    //         ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
    //             todo!()
    //         }
    //     },
    // };

    // Parse parameter data from attribute and function signature
    let params = param_fns
        .iter()
        .map(|f| {
            let name = f.sig.ident.to_string();
            let mut arguments = Vec::new();
            let mut ty = None;
            let mut default = None;
            for attr in &f.attrs {
                if attr.path().is_ident("default") {
                    default = Some(attr.parse_args::<Expr>().unwrap());
                }
            }
            for input in &f.sig.inputs {
                if let syn::FnArg::Typed(pat_type) = input {
                    match &*pat_type.ty {
                        syn::Type::Reference(ref_type) => match &*ref_type.elem {
                            syn::Type::Path(path) => {
                                if path.path.segments.iter().any(|seg| seg.ident == "AudioCtx") {
                                    arguments.push(ParameterArgumentTypes::Ctx);
                                } else if path
                                    .path
                                    .segments
                                    .iter()
                                    .any(|seg| seg.ident == "UGenFlags")
                                {
                                    arguments.push(ParameterArgumentTypes::Flags);
                                } else {
                                    panic!(
                                        "unknown argument found in parameter function: {:?}",
                                        path
                                    );
                                };
                            }
                            _ => {
                                panic!(
                                    "unknown argument found in parameter function: {:?}",
                                    ref_type
                                );
                            }
                        },
                        syn::Type::Path(path) => {
                            if path
                                .path
                                .segments
                                .iter()
                                .any(|seg| seg.ident == "f64" || seg.ident == "PFloat")
                            {
                                ty = Some(ParameterType::Float);
                                arguments
                                    .push(ParameterArgumentTypes::Parameter(ParameterType::Float));
                            } else if path.path.segments.iter().any(|seg| seg.ident == "f32") {
                                ty = Some(ParameterType::Float32);
                                arguments.push(ParameterArgumentTypes::Parameter(
                                    ParameterType::Float32,
                                ));
                            } else if path.path.segments.iter().any(|seg| seg.ident == "PInteger") {
                                ty = Some(ParameterType::Integer);
                                arguments.push(ParameterArgumentTypes::Parameter(
                                    ParameterType::Integer,
                                ));
                            } else {
                                panic!("unknown argument found in parameter function: {:?}", path);
                            };
                        }
                        _ => {
                            panic!(
                                "unknown argument found in parameter function: {:?}",
                                pat_type.ty
                            );
                        }
                    }
                }
            }

            ParameterData {
                name,
                ty: ty.unwrap_or(ParameterType::Trigger),
                arguments,
                fn_name: f.sig.ident.clone(),
            }
        })
        .collect::<Vec<_>>();
    let num_params: syn::Type = syn::parse_str(&format!("U{}", param_fns.len())).unwrap();
    let num_inputs: syn::Type = syn::parse_str(&format!("U{}", num_input_channels)).unwrap();
    let num_outputs: syn::Type = syn::parse_str(&format!("U{}", num_output_channels)).unwrap();

    let parameter_descriptions = params
        .iter()
        .map(|p| {
            let name = &p.name;
            quote! { #name , }
        })
        .collect::<Vec<_>>();
    // let parameter_types = params
    //     .iter()
    //     .map(|p| {
    //         let ty = &p.ty;
    //         quote! { #ty }
    //     })
    //     .collect();
    let parameter_calls = params
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let fn_name = &p.fn_name;
            let arguments = p.arguments.iter().map(|a| match a {
                ParameterArgumentTypes::Parameter(ty) => match ty {
                    ParameterType::Float => quote! { _value.float().unwrap() },
                    ParameterType::Float32 => quote! { _value.float().unwrap() as f32 },
                    ParameterType::Integer => quote! { _value.into() },
                    ParameterType::Trigger => quote! {},
                },
                ParameterArgumentTypes::Ctx => quote! { _ctx },
                ParameterArgumentTypes::Flags => quote! { _flags },
            });
            quote! { #index => { Self::#fn_name (self, #(#arguments),*); } }
        })
        .collect::<Vec<_>>();
    let parameter_hints = params
        .iter()
        .map(|p| match &p.ty {
            ParameterType::Float => quote! { #crate_ident::ParameterHint::float(|h| h) },
            ParameterType::Float32 => quote! { #crate_ident::ParameterHint::float(|h| h) },
            ParameterType::Integer => quote! { #crate_ident::ParameterHint::integer(|h| h) },
            ParameterType::Trigger => quote! { #crate_ident::ParameterHint::Trigger },
        })
        .collect::<Vec<_>>();

    // Remove all parsed attributes from the impl block
    let mut input = input.clone();
    for item in &mut input.items {
        if let ImplItem::Fn(method) = item {
            method.attrs.retain(|attr| {
                !attr.path().is_ident("param")
                    && !attr.path().is_ident("init")
                    && !attr.path().is_ident("process")
                    && !attr.path().is_ident("process_block")
            });
        }
    }

    let expanded = quote! {
        impl #impl_block_generics #crate_ident::UGen for #struct_name {
            type Sample = F;
            type Inputs = #num_inputs ;
            type Outputs = #num_outputs ;
            type Parameters = #num_params ;

            #init_impl
            #process_impl
            #process_block_fn


    fn param_hints()
    -> #crate_ident::numeric_array::NumericArray<#crate_ident::ParameterHint, Self::Parameters> {
        [ #(#parameter_hints),* ].into()
    }

    fn param_descriptions(
    ) -> #crate_ident::numeric_array::NumericArray<&'static str, Self::Parameters> {
        [ #(#parameter_descriptions)* ].into()
    }

    fn param_apply(
        &mut self,
        _ctx: &mut #crate_ident::AudioCtx,
        _index: usize,
        _value: #crate_ident::ParameterValue,
    ) {
                match _index {
                    #(#parameter_calls),*
                    _ => {}
                }
    }

            // #(#param_getters)*
            // #(#param_setters)*
        }
        #input
    };

    TokenStream::from(expanded)
}

enum ParameterArgumentTypes {
    Parameter(ParameterType),
    Ctx,
    Flags,
}

// TODO: Move to knaster_primitives and depend on it, both here and in knaster_core
struct ParameterData {
    name: String,
    ty: ParameterType,
    arguments: Vec<ParameterArgumentTypes>,
    fn_name: Ident,
}
enum ParameterType {
    Float,
    Float32,
    Trigger,
    Integer,
}
fn get_knaster_crate_name() -> proc_macro2::TokenStream {
    match crate_name("knaster") {
        Ok(FoundCrate::Itself) => Some(quote!(crate)),
        Ok(FoundCrate::Name(name)) => {
            let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
            Some(quote!(::#ident))
        }
        _ => None,
    }
    .unwrap_or_else(|| {
        match crate_name("knaster_graph") {
            Ok(FoundCrate::Itself) => Some(quote!(crate)),
            Ok(FoundCrate::Name(name)) => {
                let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
                Some(quote!(::#ident))
            }
            _ => None,
        }
        .unwrap_or_else(|| {
            match crate_name("knaster_core") {
                Ok(FoundCrate::Itself) => Some(quote!(crate)),
                Ok(FoundCrate::Name(name)) => {
                    let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
                    Some(quote!(::#ident))
                }
                _ => None,
            }
            .expect("Could not find knaster crate to import UGen trait and other types from.")
        })
    })
}
