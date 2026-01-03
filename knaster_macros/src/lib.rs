mod parameters;
use parameters::{gen_float_param_set_fn, parse_parameter_functions};

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::{ToTokens, quote};
use syn::{DeriveInput, Expr, ImplItem, ItemImpl, Lit, Type, parse_macro_input, spanned::Spanned};

use crate::parameters::{
    extract_float_parameters, gen_parameter_calls, gen_parameter_descriptions, gen_parameter_hints,
};

// TODO: Allow passing process and process_block functions with the same signature as for the UGen
// trait
// TODO: Copy generics from type impl to UGen trait impl
// TODO: Require float parameter functions to take a ctx argument

#[proc_macro_derive(KnasterIntegerParameter)]
pub fn knaster_integer_parameter(input: TokenStream) -> TokenStream {
    let crate_ident = get_knaster_crate_name();
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    let ident = input.ident;

    let (len, descriptions) = match input.data {
        syn::Data::Enum(enum_item) => {
            let descriptions = enum_item
                .variants
                .iter()
                .map(|v| v.ident.to_string())
                .collect::<Vec<_>>();
            (enum_item.variants.len(), descriptions)
        }
        _ => panic!("KnasterIntegerParameter only works on Enums"),
    };
    let description_matches = descriptions.iter().enumerate().map(|(i, d)| {
        quote! {
            PInteger(#i) => Some(#d),
        }
    });

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
    impl From<#crate_ident::PInteger> for #ident {
        fn from(value: #crate_ident::PInteger) -> Self {
            <Self as num_traits::FromPrimitive>::from_usize(value.0).unwrap_or( #ident ::default())
        }
    }
    impl From< #ident > for PInteger {
        fn from(value: #ident) -> Self {
            PInteger(value as usize)
        }
    }
    impl #crate_ident::PIntegerConvertible for #ident {
        fn pinteger_range() -> (PInteger, PInteger) {
            (
                PInteger(0),
                PInteger(#len),
            )
        }
        fn pinteger_descriptions(v: PInteger) -> Option<&'static str> {
                match v {
                    #(#description_matches)*
                    _ => None,
                }
        }
    }
            };

    // Hand the output tokens back to the compiler
    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn impl_ugen(_attr: TokenStream, item: TokenStream) -> TokenStream {
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
    // Overrides the associated types of the UGen trait. `Parameters` cannot be overridden
    let mut sample_type_override: Option<Type> = None;
    let mut inputs_type_override: Option<Type> = None;
    let mut outputs_type_override: Option<Type> = None;
    // Set these to true if we copy the process and/or process_block functions to the UGen impl
    let mut remove_process_fn = false;
    let mut remove_process_block_fn = false;

    for item in &input.items {
        match item {
            ImplItem::Fn(method) => {
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
            ImplItem::Type(ty) => {
                if ty.ident == "Sample" {
                    sample_type_override = Some(ty.ty.clone());
                } else if ty.ident == "Inputs" {
                    inputs_type_override = Some(ty.ty.clone());
                } else if ty.ident == "Outputs" {
                    outputs_type_override = Some(ty.ty.clone());
                }
            }
            _ => (),
        }
    }
    let sample_type = sample_type_override.unwrap_or(syn::parse_str("F").unwrap());

    let init_impl = match init_fn {
        Some(init_fn) => {
            // let init_fn_args = Vec::new();
            let init_fn_name = &init_fn.sig.ident;
            quote! {
                fn init(&mut self, _sample_rate: u32, _block_size: usize) {
                    self.#init_fn_name ( _sample_rate, _block_size );
                }
            }
        }
        None => quote! {},
    };

    let mut num_input_channels = None;
    let mut num_output_channels = None;

    let process_impl = match process_fn {
        Some(process_fn) => {
            let mut matches_ugen_process_fn_sig = true;

            match &process_fn.sig.output {
                syn::ReturnType::Default => {
                    num_output_channels = Some(0);
                    matches_ugen_process_fn_sig = false;
                }
                syn::ReturnType::Type(_rarrow, return_ty) => match &**return_ty {
                    syn::Type::Array(array) => {
                        matches_ugen_process_fn_sig = false;
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
                    syn::Type::Path(ty_path) => {
                        if !(ty_path.qself.is_none()
                            && matches!(ty_path.path.segments.last(), Some(ps) if ps.ident == "Frame" || ps.ident == "NumericArray"))
                        {
                            // matches_ugen_process_fn_sig = false;
                            return Err(syn::Error::new(
                                return_ty.span(),
                                "process function must return an array of samples or match the UGen trait process method",
                            ));
                        }
                        // else we match the UGen process dn sig
                    }
                    _ => {
                        return Err(syn::Error::new(
                            return_ty.span(),
                            "process function must return an array of samples or match the UGen trait process method",
                        ));
                    }
                },
            }

            // Check if the arguments match the UGen process fn
            for (index, input) in process_fn.sig.inputs.iter().enumerate() {
                match (index, input) {
                    (0, syn::FnArg::Receiver(rec)) => {
                        if rec.mutability.is_none() {
                            return Err(syn::Error::new(
                                rec.span(),
                                "process function must take &mut self",
                            ));
                        }
                    }
                    (index, syn::FnArg::Typed(ty)) => match &*ty.ty {
                        syn::Type::Reference(ref_type) => match &*ref_type.elem {
                            syn::Type::Path(path) => {
                                #[allow(clippy::if_same_then_else)]
                                if index == 1
                                    && !path.path.segments.iter().any(|seg| seg.ident == "AudioCtx")
                                {
                                    matches_ugen_process_fn_sig = false;
                                } else if index == 2
                                    && !path
                                        .path
                                        .segments
                                        .iter()
                                        .any(|seg| seg.ident == "UGenFlags")
                                {
                                    matches_ugen_process_fn_sig = false;
                                }
                            }
                            _ => matches_ugen_process_fn_sig = false,
                        },
                        syn::Type::Path(path) => {
                            if index == 3
                                && !path
                                    .path
                                    .segments
                                    .iter()
                                    .any(|seg| seg.ident == "Frame" || seg.ident == "NumericArray")
                            {
                                matches_ugen_process_fn_sig = false;
                            }
                        }
                        _ => matches_ugen_process_fn_sig = false,
                    },
                    _ => matches_ugen_process_fn_sig = false,
                }
            }
            let process_fn_name = &process_fn.sig.ident;
            let mut process_args = Vec::new();

            if !matches_ugen_process_fn_sig {
                for input in &process_fn.sig.inputs {
                    if let syn::FnArg::Typed(pat_type) = input {
                        match &*pat_type.ty {
                            syn::Type::Reference(ref_type) => match &*ref_type.elem {
                                syn::Type::Path(path) => {
                                    if path.path.segments.iter().any(|seg| seg.ident == "AudioCtx")
                                    {
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
                            let _input_array: [F; #num_input_channels ] = [ #(#input_array_elements)* ];
                            Self:: #process_fn_name (self,  #(#process_args)* ).into()
                    }
                }
            } else {
                remove_process_fn = true;
                quote! {
                    #process_fn
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
        Some(process_block_fn) => {
            let mut matches_ugen_process_fn_sig = true;

            // Check generics
            if process_block_fn.sig.generics.params.len() != 2 {
                matches_ugen_process_fn_sig = false;
            }
            // Check if the arguments match the UGen process fn
            for (index, input) in process_block_fn.sig.inputs.iter().enumerate() {
                match (index, input) {
                    (0, syn::FnArg::Receiver(rec)) => {
                        if rec.mutability.is_none() {
                            return Err(syn::Error::new(
                                rec.span(),
                                "process function must take &mut self",
                            ));
                        }
                    }
                    (index, syn::FnArg::Typed(ty)) => match &*ty.ty {
                        syn::Type::Reference(ref_type) => match &*ref_type.elem {
                            syn::Type::Path(path) => {
                                #[allow(clippy::if_same_then_else)]
                                if index == 1
                                    && !path.path.segments.iter().any(|seg| seg.ident == "AudioCtx")
                                {
                                    matches_ugen_process_fn_sig = false;
                                } else if index == 2
                                    && !path
                                        .path
                                        .segments
                                        .iter()
                                        .any(|seg| seg.ident == "UGenFlags")
                                {
                                    matches_ugen_process_fn_sig = false;
                                } else if index == 3
                                    && (ref_type.mutability.is_some()
                                        || !path
                                            .path
                                            .segments
                                            .iter()
                                            .any(|seg| seg.ident == "InBlock"))
                                {
                                    matches_ugen_process_fn_sig = false;
                                } else if index == 4
                                    && (ref_type.mutability.is_none()
                                        || !path
                                            .path
                                            .segments
                                            .iter()
                                            .any(|seg| seg.ident == "OutBlock"))
                                {
                                    matches_ugen_process_fn_sig = false;
                                } else if index > 4 {
                                    matches_ugen_process_fn_sig = false;
                                }
                            }
                            _ => matches_ugen_process_fn_sig = false,
                        },
                        syn::Type::Path(path) => {
                            if index == 3
                                && !path
                                    .path
                                    .segments
                                    .iter()
                                    .any(|seg| seg.ident == "Frame" || seg.ident == "NumericArray")
                            {
                                matches_ugen_process_fn_sig = false;
                            }
                        }
                        _ => matches_ugen_process_fn_sig = false,
                    },
                    _ => matches_ugen_process_fn_sig = false,
                }
            }
            if !matches_ugen_process_fn_sig {
                let mut process_args = Vec::new();

                let process_block_fn_name = &process_block_fn.sig.ident;
                for input in &process_block_fn.sig.inputs {
                    if let syn::FnArg::Typed(pat_type) = input {
                        match &*pat_type.ty {
                            syn::Type::Reference(ref_type) => match &*ref_type.elem {
                                syn::Type::Path(path) => {
                                    if path.path.segments.iter().any(|seg| seg.ident == "AudioCtx")
                                    {
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
                                                    return Err(syn::Error::new(
                                                        ref_type2.span(),
                                                        "number of output chanels in process and process_block methods don't match",
                                                    ));
                                                }
                                            }
                                            process_args.push(quote! { output_array, });
                                        } else {
                                            if let Some(num_input_channels) = num_input_channels {
                                                if num_channels != num_input_channels {
                                                    return Err(syn::Error::new(
                                                        ref_type2.span(),
                                                        "number of input chanels in process and process_block methods don't match",
                                                    ));
                                                }
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

                // let input_array = if num_input_channels == 0 {
                //         quote!{}
                //     } else {
                //         quote! {
                //         let input_array: [&[F]; #num_input_channels ] = [ #(#input_array_elements)* ];
                //         }
                //     };
                // let output_array = if num_output_channels == 0 {
                //         quote!{}
                //     } else {
                //         quote! {
                //         let mut outputs = output.iter_mut();
                //         let output_array: [&mut[F]; #num_output_channels ]= [ #(#output_array_elements)* ];
                //         }
                //     };

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
                        let input_array: [&[F]; #num_input_channels ] = [ #(#input_array_elements)* ];
                        let mut outputs = output.iter_mut();
                        let output_array: [&mut[F]; #num_output_channels ]= [ #(#output_array_elements)* ];
                        Self:: #process_block_fn_name (self,  #(#process_args)* ).into()
                    }
                }
            } else {
                // The provided function matches the UGen process_block, paste it as is
                remove_process_block_fn = true;
                quote! { #process_block_fn }
            }
        }
        None => quote! {},
    };

    let params = parse_parameter_functions(param_fns, &sample_type)?;

    let parameter_descriptions = gen_parameter_descriptions(&params);
    let parameter_calls = gen_parameter_calls(&params);
    let parameter_hints = gen_parameter_hints(&params);

    let float_parameter_data = extract_float_parameters(&params);

    let float_param_set_fn = gen_float_param_set_fn(&float_parameter_data, struct_name);

    let num_input_channels = num_input_channels.unwrap_or(0);
    let num_output_channels = num_output_channels.unwrap_or(0);
    let num_float_params: syn::Type =
        syn::parse_str(&format!("U{}", float_parameter_data.len())).unwrap();
    let num_params: syn::Type = syn::parse_str(&format!("U{}", params.len())).unwrap();
    let num_inputs: syn::Type = syn::parse_str(&format!("U{num_input_channels}")).unwrap();
    let num_outputs: syn::Type = syn::parse_str(&format!("U{num_output_channels}")).unwrap();

    // Remove all parsed attributes from the impl block
    let mut input = input.clone();
    let mut items_to_remove = vec![];
    for (index, item) in &mut input.items.iter_mut().enumerate() {
        if let ImplItem::Fn(method) = item {
            if remove_process_fn
                && (method.sig.ident == "process"
                    || method
                        .attrs
                        .iter()
                        .any(|attr| attr.path().is_ident("process")))
            {
                items_to_remove.push(index);
            }
            if remove_process_block_fn
                && (method.sig.ident == "process_block"
                    || method
                        .attrs
                        .iter()
                        .any(|attr| attr.path().is_ident("process_block")))
            {
                items_to_remove.push(index);
            }
            method.attrs.retain(|attr| {
                !attr.path().is_ident("param")
                    && !attr.path().is_ident("init")
                    && !attr.path().is_ident("process")
                    && !attr.path().is_ident("process_block")
            });
        }
    }
    for i in items_to_remove.into_iter().rev() {
        input.items.remove(i);
    }

    // Remove associated types
    input.items.retain(|item| {
        if let ImplItem::Type(ty) = item {
            !matches!(
                ty.ident.to_string().as_ref(),
                "Sample" | "Inputs" | "Outputs"
            )
        } else {
            true
        }
    });

    // Insert new parameter functions
    for p in params {
        if let Some(generated_function) = p.generated_function {
            input.items.push(ImplItem::Verbatim(generated_function));
        }
    }

    let atype_sample = quote! {type Sample = #sample_type ;};
    let atype_inputs = inputs_type_override
        .map(|ty| quote! { type Inputs = #ty ; })
        .unwrap_or(quote! {type Inputs = #crate_ident::typenum::#num_inputs ;});
    let atype_outputs = outputs_type_override
        .map(|ty| quote! { type Outputs = #ty ; })
        .unwrap_or(quote! {type Outputs = #crate_ident::typenum::#num_outputs ;});

    let unknown_parameter_error = format!(
        "Unknown parameter set for {}:",
        struct_name.into_token_stream()
    );
    let error_parameter_smoothing = format!(
        "Error setting parameter smoothing for {}, wrapper does not exist. Index: ",
        struct_name.into_token_stream()
    );

    Ok(quote! {
        impl #impl_block_generics #crate_ident::UGen for #struct_name {
            #atype_sample
            #atype_inputs
            #atype_outputs
            type FloatParameters = #crate_ident::typenum::#num_float_params ;
            type Parameters = #crate_ident::typenum::#num_params ;

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

            #float_param_set_fn

            fn param_apply(
                &mut self,
                ctx: &mut #crate_ident::AudioCtx,
                _index: usize,
                _value: #crate_ident::ParameterValue,
            ) {
                if let #crate_ident::ParameterValue::Smoothing(_, _) =_value  {
                    #crate_ident::rt_log!(ctx.logger(); #error_parameter_smoothing, _index);
                }
                match _index {
                    #(#parameter_calls),*
                    _ => {
                        #crate_ident::rt_log!(ctx.logger(); #unknown_parameter_error, _index);
                    }
                }
            }
        }
        #input
    })
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
