//! Parsing of parameter functions and generation of related functions in the output code

use std::cmp::Ordering;

use darling::FromMeta;
use knaster_primitives::FloatParameterKind;
use quote::{ToTokens, format_ident, quote};
use syn::{Expr, ExprRange, Ident, ImplItemFn, Type, spanned::Spanned};

use crate::get_knaster_crate_name;

#[derive(Copy, Clone, Debug)]
pub enum ParameterArgumentTypes {
    Parameter(ParameterType),
    Ctx,
}

#[derive(Debug, FromMeta)]
pub struct ParameterAttribute {
    default: Option<syn::Expr>,
    range: Option<syn::Expr>,
    kind: Option<syn::Path>,
    logarithmic: Option<bool>,
    from: Option<syn::Path>,
}

pub struct FloatParameterData {
    _name: String,
    fn_name: Ident,
    _default: Option<Expr>,
    _range: Option<ExprRange>,
    _kind: Option<FloatParameterKind>,
    _logarithmic: bool,
}

// TODO: Move to knaster_primitives and depend on it, both here and in knaster_core
pub struct ParameterData {
    name: String,
    ty: ParameterType,
    arguments: Vec<ParameterArgumentTypes>,
    fn_name: Ident,
    default: Option<Expr>,
    range: Option<ExprRange>,
    kind: Option<FloatParameterKind>,
    from_pinteger_convertible: Option<syn::Path>,
    logarithmic: bool,
    /// If a function needs to be generated for this parameter, this is the code for it.
    /// Currently only used for float parameters.
    pub generated_function: Option<proc_macro2::TokenStream>,
}
impl ParameterData {
    pub fn is_float_parameter(&self) -> bool {
        match self.ty {
            ParameterType::SampleType
            | ParameterType::Float32
            | ParameterType::PFloat
            | ParameterType::Float64 => true,
            _ => false,
        }
    }
    /// Converts the parameter from any float type to the correct float type.  
    pub fn convert_float_parameter_to_match_trait(&mut self, sample_type: &syn::Type) {
        if !self.is_float_parameter() {
            return;
        }
        let crate_ident = get_knaster_crate_name();
        let mut matches_sig = true;
        matches_sig = matches_sig && matches!(self.ty, ParameterType::SampleType);
        matches_sig = matches_sig
            && self.arguments.len() >= 2
            && matches!(self.arguments[1], ParameterArgumentTypes::Ctx);
        if matches_sig {
            // Everything already matches
            return;
        }
        // Generate a new function with the correct signature
        let old_fn_name = self.fn_name.clone();
        let new_fn_name = format_ident!("{}_ugen_generated", old_fn_name);
        let value_conversion = match self.ty {
            ParameterType::Float32 => quote! { value.to_f32() },
            ParameterType::Float64 => {
                quote! { value.to_f64() }
            }
            ParameterType::PFloat => {
                quote! { value.to_f64() as PFloat}
            }
            ParameterType::SampleType => quote! { value },
            _ => unreachable!(),
        };
        let call = if self.arguments.len() == 1 {
            quote! { self.#old_fn_name( #value_conversion ); }
        } else {
            match (self.arguments[0], self.arguments[1]) {
                (ParameterArgumentTypes::Parameter(_), ParameterArgumentTypes::Ctx) => {
                    quote! { self.#old_fn_name( #value_conversion , _ctx); }
                }
                (ParameterArgumentTypes::Ctx, ParameterArgumentTypes::Parameter(_)) => {
                    quote! { self.#old_fn_name( _ctx, #value_conversion); }
                }
                _ => {
                    unreachable!(
                        "Unsupported param function signature should have been caught earlier"
                    )
                }
            }
        };
        let new_fn_body = quote! {
            fn #new_fn_name(&mut self, value: #sample_type, _ctx: &mut #crate_ident::AudioCtx) {
                #call
            }
        };
        self.generated_function = Some(new_fn_body);
        // This should now be set to the new function that matches the signature expected by UGen
        self.fn_name = new_fn_name;
        self.ty = ParameterType::SampleType;
        self.arguments = vec![
            ParameterArgumentTypes::Parameter(ParameterType::SampleType),
            ParameterArgumentTypes::Ctx,
        ];
    }
}
#[derive(Copy, Clone, Debug)]
pub enum ParameterType {
    /// Same as the sample type of the UGen, defaults to F
    SampleType,
    PFloat,
    Float64,
    Float32,
    Trigger,
    Integer,
    Bool,
}

pub fn parse_parameter_functions(
    param_fns: Vec<&ImplItemFn>,
    sample_type: &syn::Type,
) -> syn::Result<Vec<ParameterData>> {
    // Parse parameter data from attribute and function signature
    let mut params = Vec::new();
    for f in param_fns {
        let name = f.sig.ident.to_string();
        let mut pdata = ParameterData {
            name,
            ty: ParameterType::Trigger,
            arguments: vec![],
            fn_name: f.sig.ident.clone(),
            default: None,
            range: None,
            kind: None,
            logarithmic: false,
            from_pinteger_convertible: None,
            generated_function: None,
        };
        let mut attrs = None;
        for attr in &f.attrs {
            if attr.path().is_ident("param") {
                if let syn::Meta::List(list) = attr.meta.clone() {
                    let attr_args = match darling::ast::NestedMeta::parse_meta_list(list.tokens) {
                        Ok(v) => v,
                        Err(e) => {
                            return Err(syn::Error::new(
                                attr.span(),
                                format!("Failed to parse param attribute: {e}"),
                            ));
                        }
                    };

                    attrs = match ParameterAttribute::from_list(&attr_args) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            return Err(syn::Error::new(
                                attr.span(),
                                format!("Failed to parse param attribute: {e}"),
                            ));
                        }
                    };
                }
            }
        }
        let mut num_parameter_value_arguments = 0;
        for input in &f.sig.inputs {
            if let syn::FnArg::Typed(pat_type) = input {
                if pat_type.ty.as_ref() == sample_type {
                    pdata.ty = ParameterType::SampleType;
                    pdata
                        .arguments
                        .push(ParameterArgumentTypes::Parameter(ParameterType::Float32));
                    num_parameter_value_arguments += 1;
                    continue;
                }
                match &*pat_type.ty {
                    syn::Type::Reference(ref_type) => match &*ref_type.elem {
                        syn::Type::Path(path) => {
                            if path.path.segments.iter().any(|seg| seg.ident == "AudioCtx") {
                                pdata.arguments.push(ParameterArgumentTypes::Ctx);
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
                        if num_parameter_value_arguments > 0 {
                            return Err(syn::Error::new(
                                input.span(),
                                "each parameter function can only take one parameter value as argument",
                            ));
                        }
                        if path.path.segments.iter().any(|seg| seg.ident == "PFloat") {
                            if cfg!(feature = "fail_on_parameter_signature_conversion") {
                                return Err(syn::Error::new(
                                    input.span(),
                                    "Float parameter type is not the same as the sample type",
                                ));
                            }
                            pdata.ty = ParameterType::PFloat;
                            pdata
                                .arguments
                                .push(ParameterArgumentTypes::Parameter(ParameterType::PFloat));
                            num_parameter_value_arguments += 1;
                        } else if path.path.segments.iter().any(|seg| seg.ident == "f32") {
                            if cfg!(feature = "fail_on_parameter_signature_conversion") {
                                return Err(syn::Error::new(
                                    input.span(),
                                    "Float parameter type is not the same as the sample type",
                                ));
                            }
                            pdata.ty = ParameterType::Float32;
                            pdata
                                .arguments
                                .push(ParameterArgumentTypes::Parameter(ParameterType::Float32));
                            num_parameter_value_arguments += 1;
                        } else if path.path.segments.iter().any(|seg| seg.ident == "f64") {
                            if cfg!(feature = "fail_on_parameter_signature_conversion") {
                                return Err(syn::Error::new(
                                    input.span(),
                                    "Float parameter type is not the same as the sample type",
                                ));
                            }
                            pdata.ty = ParameterType::Float64;
                            pdata
                                .arguments
                                .push(ParameterArgumentTypes::Parameter(ParameterType::Float64));
                            num_parameter_value_arguments += 1;
                        } else if path.path.segments.iter().any(|seg| seg.ident == "PInteger") {
                            pdata.ty = ParameterType::Integer;
                            pdata
                                .arguments
                                .push(ParameterArgumentTypes::Parameter(ParameterType::Integer));
                            num_parameter_value_arguments += 1;
                        } else if path.path.segments.iter().any(|seg| seg.ident == "bool") {
                            pdata.ty = ParameterType::Bool;
                            pdata
                                .arguments
                                .push(ParameterArgumentTypes::Parameter(ParameterType::Bool));
                            num_parameter_value_arguments += 1;
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

        if let Some(attrs) = attrs {
            if let Some(akind) = attrs.kind {
                if !matches!(
                    pdata.ty,
                    ParameterType::SampleType
                        | ParameterType::Float32
                        | ParameterType::PFloat
                        | ParameterType::Float64
                ) {
                    return Err(syn::Error::new(
                        akind.span(),
                        "`kind` is only supported for float parameters. Use `from` to derive PInteger hints from a PIntegerConvertible type.",
                    ));
                }
                pdata.kind = {
                    if akind.segments.len() != 1 {
                        return Err(syn::Error::new(akind.span(), "Invalid parameter kind"));
                    }
                    let ident = akind.segments.first().unwrap().ident.to_string();
                    match ident.as_str() {
                        "Frequency" => Some(FloatParameterKind::Frequency),
                        "Amplitude" => Some(FloatParameterKind::Amplitude),
                        "Q" => Some(FloatParameterKind::Q),
                        "Seconds" => Some(FloatParameterKind::Seconds),
                        _ => {
                            return Err(syn::Error::new(akind.span(), "Invalid parameter kind"));
                        }
                    }
                };
            }
            if let Some(from) = attrs.from {
                if !matches!(pdata.ty, ParameterType::Integer) {
                    return Err(syn::Error::new(
                        from.span(),
                        "`from` is only supported for integer parameters.",
                    ));
                }
                pdata.from_pinteger_convertible = Some(from);
            }
            if let Some(Expr::Range(range)) = attrs.range {
                if range.start.is_none() {
                    return Err(syn::Error::new(
                        range.span(),
                        "Parameter range must have a start value",
                    ));
                }
                if range.end.is_none() {
                    return Err(syn::Error::new(
                        range.span(),
                        "Parameter range must have an end value",
                    ));
                }
                if let syn::RangeLimits::HalfOpen(_) = range.limits {
                    return Err(syn::Error::new(
                        range.span(),
                        "Parameter range must be inclusive/closed",
                    ));
                }
                pdata.range = Some(range);
            }
            pdata.default = attrs.default;
            if let Some(logarithmic) = attrs.logarithmic {
                pdata.logarithmic = logarithmic;
            }
        }

        params.push(pdata)
    }
    // For float parameters with function signatures that don't match the expected signature,
    // generate a new function with the same body, but with the correct signature.
    for p in &mut params {
        p.convert_float_parameter_to_match_trait(sample_type);
    }

    // Sort the parameters so that the float parameters are first
    params.sort_by(|a, b| match (&a.ty, &b.ty) {
        (ParameterType::SampleType, ParameterType::SampleType) => Ordering::Equal,
        (ParameterType::SampleType, _) => Ordering::Less,
        (_, ParameterType::SampleType) => Ordering::Greater,
        _ => Ordering::Equal,
    });
    Ok(params)
}

pub fn gen_float_param_set_fn(
    float_parameter_data: &[FloatParameterData],
    struct_name: &Type,
) -> proc_macro2::TokenStream {
    let parameter_calls = float_parameter_data
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let fn_name = &p.fn_name;
            quote! { #index => { Self::#fn_name } }
        })
        .collect::<Vec<_>>();
    let crate_ident = get_knaster_crate_name();
    let unknown_parameter_error = format!(
        "Unknown parameter set for {}:",
        struct_name.to_token_stream()
    );
    let unknown_parameter_error_panic = format!(
        "Unknown parameter set for {}: {{}}",
        struct_name.to_token_stream()
    );
    quote! {
        fn float_param_set_fn(
            &mut self,
            ctx: &mut #crate_ident::AudioCtx,
            index: usize,
        ) -> fn(ugen: &mut Self, value: Self::Sample, ctx: &mut #crate_ident::AudioCtx) {
                match index {
                    #(#parameter_calls),*
                    _ => {
                        #crate_ident::rt_log!(ctx.logger(); #unknown_parameter_error, index);
                        panic!(#unknown_parameter_error_panic, index);
                    }
                }
        }
    }
}

pub fn gen_parameter_calls(params: &[ParameterData]) -> Vec<proc_macro2::TokenStream> {
    params.iter()
        .enumerate()
        .map(|(index, p)| {
            let fn_name = &p.fn_name;
            let arguments = p.arguments.iter().map(|a| match a {
                ParameterArgumentTypes::Parameter(ty) => match ty {
                    ParameterType::SampleType => quote! { _value.f().expect("parameter value is expected to be a float") },
                    ParameterType::PFloat => quote! { _value.float().expect("parameter value is expected to be a float") },
                    ParameterType::Float64 => quote! { _value.float().expect("parameter value is expected to be a float") as f64 },
                    ParameterType::Float32 => quote! { _value.float().expect("parameter value is expected to be a float") as f32 },
                    ParameterType::Integer => quote! { _value.integer().expect("parameter value is expected to be an integer") },
                    ParameterType::Bool => quote! { _value.bool().expect("parameter value is expected to be a boolean") },
                    ParameterType::Trigger => quote! {},
                },
                ParameterArgumentTypes::Ctx => quote! { ctx },
            });
            quote! { #index => { Self::#fn_name (self, #(#arguments),*); } }
        })
        .collect::<Vec<_>>()
}
pub fn gen_parameter_hints(params: &[ParameterData]) -> Vec<proc_macro2::TokenStream> {
    let crate_ident = get_knaster_crate_name();
    params
        .iter()
        .map(|p| match &p.ty {
            ParameterType::SampleType | ParameterType::Float32 | ParameterType::PFloat | ParameterType::Float64 => {
                // let kind = if let Some(kind) = &p.kind {
                //     quote! { .kind }
                // } else {
                //     quote! {}
                // };
                let range = if let Some(range) = &p.range {
                    quote! { .range(#range) }
                } else {
                    quote! {}
                };
                let kind = if let Some(kind) = &p.kind {
                    let kind = match kind {
                        FloatParameterKind::Amplitude => quote! { #crate_ident::FloatParameterKind::Amplitude },
                        FloatParameterKind::Frequency => quote! { #crate_ident::FloatParameterKind::Frequency },
                        FloatParameterKind::Q => quote! { #crate_ident::FloatParameterKind::Q },
                        FloatParameterKind::Seconds => quote! { #crate_ident::FloatParameterKind::Seconds },
                    };
                    quote! { .kind(#kind) }
                } else {
                    quote! {}
                };
                let logarithmic = if p.logarithmic {
                    quote! { .logarithmic(true) }
                } else {
                    quote! {}
                };
                let default = if let Some(default) = &p.default {
                    quote! { .default(#default) }
                } else {
                    quote! {}
                };
                quote! { #crate_ident::ParameterHint::new_float(|h| h #range #kind #logarithmic #default ) }
            }
            ParameterType::Integer => {
                if p.from_pinteger_convertible.is_some() {
                    let from = p.from_pinteger_convertible.as_ref().unwrap();
                    quote! { #crate_ident::ParameterHint::from_pinteger_enum::<#from>() }
                } else {
                    let range = if let Some(range) = &p.range {
                        quote! { #range }
                    } else {
                        quote! { (#crate_ident::PInteger::MIN, #crate_ident::PInteger::MAX) }
                    };
                    quote! { #crate_ident::ParameterHint::new_integer(#range , |h| h) }
                }
            }
            ParameterType::Trigger => quote! { #crate_ident::ParameterHint::Trigger },
            ParameterType::Bool => quote! { #crate_ident::ParameterHint::Bool },
        })
        .collect::<Vec<_>>()
}

pub fn gen_parameter_descriptions(params: &[ParameterData]) -> Vec<proc_macro2::TokenStream> {
    params
        .iter()
        .map(|p| {
            let name = &p.name;
            quote! { #name , }
        })
        .collect::<Vec<_>>()
}

pub fn extract_float_parameters(params: &[ParameterData]) -> Vec<FloatParameterData> {
    params
        .iter()
        .filter_map(|p| match p.ty {
            ParameterType::SampleType => Some(FloatParameterData {
                _name: p.name.clone(),
                fn_name: p.fn_name.clone(),
                _default: p.default.clone(),
                _range: p.range.clone(),
                _kind: p.kind,
                _logarithmic: p.logarithmic,
            }),
            _ => None,
        })
        .collect::<Vec<_>>()
}
