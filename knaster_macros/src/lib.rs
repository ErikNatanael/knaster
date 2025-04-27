extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::quote;
use syn::{
    DeriveInput, Expr, Ident, ImplItem, ImplItemFn, ItemImpl, Lit, parse_macro_input,
    spanned::Spanned,
};

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
    let input = parse_macro_input!(item as ItemImpl);
    parse_ugen_impl(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
fn parse_ugen_impl(mut input: ItemImpl) -> syn::Result<proc_macro2::TokenStream> {
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

    let mut num_input_channels = None;
    let mut num_output_channels = None;

    let process_impl = match process_fn {
        Some(process_fn) => {
            match &process_fn.sig.output {
                syn::ReturnType::Default => num_output_channels = Some(0),
                syn::ReturnType::Type(_rarrow, return_ty) => match &**return_ty {
                    syn::Type::Array(array) => {
                        num_output_channels = if let Expr::Lit(el) = &array.len {
                            if let Lit::Int(i) = &el.lit {
                                Some(i.base10_parse().unwrap())
                            } else {
                                return Err(syn::Error::new(
                                    el.span(),
                                    "process function must return an array of samples with a literal length",
                                ));
                            }
                        } else {
                            return Err(syn::Error::new(
                                array.span(),
                                "process function must return an array of samples with a literal length",
                            ));
                        };
                    }
                    _ => {
                        return Err(syn::Error::new(
                            return_ty.span(),
                            "process function must return an array of samples",
                        ));
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
                                    return Err(syn::Error::new(
                                        path.span(),
                                        "unknown argument found in process function",
                                    ));
                                };
                            }
                            _ => {
                                return Err(syn::Error::new(
                                    ref_type.span(),
                                    "unknown argument found in process function",
                                ));
                            }
                        },

                        syn::Type::Array(array) => {
                            num_input_channels = if let Expr::Lit(el) = &array.len {
                                if let Lit::Int(i) = &el.lit {
                                    Some(i.base10_parse().unwrap())
                                } else {
                                    return Err(syn::Error::new(
                                        el.span(),
                                        "process function input array must use a literal length",
                                    ));
                                }
                            } else {
                                return Err(syn::Error::new(
                                    array.span(),
                                    "process function input array must use a literal length",
                                ));
                            };
                            process_args.push(quote! { _input_array, });
                        }
                        _ => {
                            return Err(syn::Error::new(
                                pat_type.span(),
                                "unknown argument found in process function",
                            ));
                        }
                    }
                }
            }
            let num_input_channels = num_input_channels.unwrap_or(0);
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

    let process_block_impl = match process_block_fn {
        Some(process_fn) => {
            let mut process_args = Vec::new();

            let process_block_fn_name = &process_fn.sig.ident;
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
                                    return Err(syn::Error::new(
                                        path.span(),
                                        "unknown argument in process_block function",
                                    ));
                                };
                            }
                            _ => {
                                return Err(syn::Error::new(
                                    pat_type.span(),
                                    "unknown argument in process_block function",
                                ));
                            }
                        },
                        // [&[F]; N] is input, [&mut [F]; N] is output
                        syn::Type::Array(array) => {
                            let num_channels: usize = if let Expr::Lit(el) = &array.len {
                                if let Lit::Int(i) = &el.lit {
                                    i.base10_parse().unwrap()
                                } else {
                                    return Err(syn::Error::new(
                                        el.span(),
                                        "process function input array must use a literal length",
                                    ));
                                }
                            } else {
                                return Err(syn::Error::new(
                                    array.span(),
                                    "process function input array must use a literal length",
                                ));
                            };
                            match array.elem.as_ref() {
                                syn::Type::Reference(ref_type2) => {
                                    if ref_type2.mutability.is_some() {
                                        if let Some(num_output_channels) = num_output_channels {
                                            if num_channels != num_output_channels {
                                                panic!(
                                                    "number of output chanels in process and process_block methods don't match"
                                                );
                                            }
                                        }
                                        process_args.push(quote! { output_array, });
                                    } else if let Some(num_input_channels) = num_input_channels {
                                        if num_channels != num_input_channels {
                                            panic!(
                                                "number of input chanels in process and process_block methods don't match"
                                            );
                                        }
                                        process_args.push(quote! { input_array, });
                                    }
                                    // TODO: check that it's a slice of F/f32/f64
                                }
                                _ => {
                                    return Err(syn::Error::new(
                                        input.span(),
                                        "unknown argument found in process_block function",
                                    ));
                                }
                            }
                        }

                        _ => {
                            return Err(syn::Error::new(
                                input.span(),
                                "unknown argument found in process_block function",
                            ));
                        }
                    }
                }
            }
            let num_input_channels = num_input_channels.unwrap_or(0);
            let input_array_elements = (0..num_input_channels)
                .map(|i: usize| quote! { input.channel_as_slice( #i ), })
                .collect::<Vec<_>>();
            let num_output_channels = num_output_channels.unwrap_or(0);
            let output_array_elements = (0..num_output_channels)
                .map(|_i: usize| quote! { outputs.next().unwrap(), })
                .collect::<Vec<_>>();

            quote! {

            fn process_block<InBlock, OutBlock>(
                &mut self,
                _ctx: &mut #crate_ident::AudioCtx,
                _flags: &mut #crate_ident::UGenFlags,
                input: &InBlock,
                output: &mut OutBlock,
            ) where
                InBlock: #crate_ident::BlockRead<Sample = Self::Sample>,
                OutBlock: #crate_ident::Block<Sample = Self::Sample>,
            {
                            let input_array = [ #(#input_array_elements)* ];
                    let mut outputs = output.iter_mut();
                    let output_array = [ #(#output_array_elements)* ];
                            Self:: #process_block_fn_name (self,  #(#process_args)* ).into()
                        }
                    }
        }
        None => quote! {},
    };

    let num_input_channels = num_input_channels.unwrap_or(0);
    let num_output_channels = num_output_channels.unwrap_or(0);
    let num_params: syn::Type = syn::parse_str(&format!("U{}", param_fns.len())).unwrap();
    let num_inputs: syn::Type = syn::parse_str(&format!("U{}", num_input_channels)).unwrap();
    let num_outputs: syn::Type = syn::parse_str(&format!("U{}", num_output_channels)).unwrap();

    let params = parse_parameter_functions(param_fns)?;

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

    Ok(quote! {
        impl #impl_block_generics #crate_ident::UGen for #struct_name {
            type Sample = F;
            type Inputs = #num_inputs ;
            type Outputs = #num_outputs ;
            type Parameters = #num_params ;

            #init_impl
            #process_impl
            #process_block_impl


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
    })
}

fn parse_parameter_functions(param_fns: Vec<&ImplItemFn>) -> syn::Result<Vec<ParameterData>> {
    // Parse parameter data from attribute and function signature
    let mut params = Vec::new();
    for f in param_fns {
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
                                return Err(syn::Error::new(
                                    input.span(),
                                    "unknown argument in parameter function",
                                ));
                            };
                        }
                        _ => {
                            return Err(syn::Error::new(
                                input.span(),
                                "unknown argument in parameter function",
                            ));
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
                            arguments.push(ParameterArgumentTypes::Parameter(ParameterType::Float));
                        } else if path.path.segments.iter().any(|seg| seg.ident == "f32") {
                            ty = Some(ParameterType::Float32);
                            arguments
                                .push(ParameterArgumentTypes::Parameter(ParameterType::Float32));
                        } else if path.path.segments.iter().any(|seg| seg.ident == "PInteger") {
                            ty = Some(ParameterType::Integer);
                            arguments
                                .push(ParameterArgumentTypes::Parameter(ParameterType::Integer));
                        } else {
                            return Err(syn::Error::new(
                                input.span(),
                                "unknown argument in parameter function",
                            ));
                        };
                    }
                    _ => {
                        return Err(syn::Error::new(
                            input.span(),
                            "unknown argument in parameter function",
                        ));
                    }
                }
            }
        }

        params.push(ParameterData {
            name,
            ty: ty.unwrap_or(ParameterType::Trigger),
            arguments,
            fn_name: f.sig.ident.clone(),
        })
    }
    Ok(params)
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
