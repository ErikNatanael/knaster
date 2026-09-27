use darling::FromMeta;
use knaster_primitives::{ControlPurpose, Float};
use quote::quote;
use syn::{Expr, Ident, ImplItemFn, spanned::Spanned};

pub enum ControlArgumentTypes {
    Value,
    Ctx,
}

// TODO: Move to knaster_primitives and depend on it, both here and in knaster_core
pub struct ControlData {
    name: String,
    arguments: Vec<ControlArgumentTypes>,
    fn_name: Ident,
    default: Expr,
    range: Option<proc_macro2::TokenStream>,
    purpose: Option<ControlPurpose>,
    logarithmic: bool,
}

#[derive(Debug, FromMeta)]
pub struct ControlAttribute {
    default: syn::Expr,
    range: Option<syn::Expr>,
    purpose: Option<syn::Path>,
    logarithmic: Option<bool>,
}

/// Parse a function marked by the `#[control]` attribute.
///
/// Can take parameters as defined in ControlAttribute
pub fn parse_control_functions(
    control_fns: Vec<&ImplItemFn>,
    crate_ident: &proc_macro2::TokenStream,
) -> syn::Result<Vec<ControlData>> {
    // Parse control data from attribute and function signature
    let mut controls = Vec::new();
    for f in control_fns {
        let name = f.sig.ident.to_string();
        let mut attrs = None;
        for attr in &f.attrs {
            if attr.path().is_ident("control") {
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

                    attrs = match ControlAttribute::from_list(&attr_args) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            return Err(syn::Error::new(
                                attr.span(),
                                format!("Failed to parse param attribute: {e}"),
                            ));
                        }
                    };
                } else {
                    return Err(syn::Error::new(
                        attr.span(),
                        format!("default value required in control attribute"),
                    ));
                }
            }
        }
        let Some(attrs) = attrs else {
            return Err(syn::Error::new(
                f.span(),
                format!("Failed to parse control attribute"),
            ));
        };
        let mut num_control_value_arguments = 0;
        let mut arguments = vec![];
        for input in &f.sig.inputs {
            if let syn::FnArg::Typed(pat_type) = input {
                match &*pat_type.ty {
                    syn::Type::Reference(ref_type) => match &*ref_type.elem {
                        syn::Type::Path(path) => {
                            if path.path.segments.iter().any(|seg| seg.ident == "AudioCtx") {
                                arguments.push(ControlArgumentTypes::Ctx);
                            } else {
                                return Err(syn::Error::new(
                                    input.span(),
                                    "unknown argument in control function",
                                ));
                            };
                        }
                        _ => {
                            return Err(syn::Error::new(
                                input.span(),
                                "unknown argument in control function",
                            ));
                        }
                    },
                    syn::Type::Path(_path) => {
                        if num_control_value_arguments > 0 {
                            return Err(syn::Error::new(
                                input.span(),
                                "each control function can only take one value as argument",
                            ));
                        }
                        arguments.push(ControlArgumentTypes::Value);
                        // TODO: Match the given Sample type somehow
                        num_control_value_arguments += 1;
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
        let purpose = attrs
            .purpose
            .map(|apurpose| -> Result<ControlPurpose, syn::Error> {
                if apurpose.segments.len() != 1 {
                    Err(syn::Error::new(apurpose.span(), "Invalid parameter kind"))
                } else {
                    let ident = apurpose.segments.first().unwrap().ident.to_string();
                    let p = match ident.as_str() {
                        "Frequency" => ControlPurpose::Frequency,
                        "Phase" => ControlPurpose::Phase,
                        "Amplitude" => ControlPurpose::Amplitude,
                        "Q" => ControlPurpose::Q,
                        "ReciprocalQ" => ControlPurpose::ReciprocalQ,
                        "Seconds" => ControlPurpose::Seconds,
                        "Offset" => ControlPurpose::Offset,
                        "Pan" => ControlPurpose::Pan,
                        "Decibel" => ControlPurpose::Decibel,
                        _ => {
                            return Err(syn::Error::new(
                                apurpose.span(),
                                "Invalid control purpose",
                            ));
                        }
                    };
                    Ok(p)
                }
            })
            .transpose()?;
        let range = attrs
            .range
            .map(|range| parse_control_range(range, crate_ident))
            .transpose()?;
        let default = attrs.default;
        let logarithmic = attrs.logarithmic.unwrap_or(false);

        let data = ControlData {
            name,
            arguments,
            fn_name: f.sig.ident.clone(),
            default,
            range,
            purpose,
            logarithmic,
        };
        controls.push(data)
    }
    Ok(controls)
}
pub fn generate_controls_methods(
    controls: Vec<ControlData>,
    crate_ident: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let control_calls = controls
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let fn_name = &p.fn_name;
            let arguments = p.arguments.iter().map(|a| match a {
                ControlArgumentTypes::Value => quote! { _value },
                ControlArgumentTypes::Ctx => quote! { ctx },
            });
            quote! { #index => { Self::#fn_name (self, #(#arguments),*); } }
        })
        .collect::<Vec<_>>();
    let control_metadatas = controls
        .into_iter()
        .map(|p| {
            let name = &p.name;
            let default = &p.default;
            let range = p
                .range
                .unwrap_or(quote! { #crate_ident::ControlRange::Infinity });
            // ControlMetadata::new("freq", F::new(440.), ControlRange::Nyquist)
            //  .with_purpose(ControlPurpose::Frequency),
            if let Some(purpose) = p.purpose {
                let purpose = purpose.to_tokens(crate_ident);
                quote! { 
                    #crate_ident::controls::ControlMetadata::new( #name , F::new(#default) , #range )
                        .with_purpose( #purpose )
                }
            } else {
                quote! { #crate_ident::controls::ControlMetadata::new( #name , F::new(#default) , #range ) }
            }
        })
        .collect::<Vec<_>>();

    Ok(quote! {
            /// Get the metadata for every control of this UGen
            fn control_metadata(&self) -> #crate_ident::numeric_array::NumericArray<#crate_ident::controls::ControlMetadata<Self::Sample>, Self::Controls> {
                [ #(#control_metadatas),* ].into()
            }
            /// Set the value of a control (float parameter)
            fn control(&mut self, ctx: &mut #crate_ident::AudioCtx, _index: usize, _value: Self::Sample) {
                match _index {
                    #(#control_calls),*
                    _ => {
                        #crate_ident::rt_log!(ctx.logger(); "Unknown parameter set for #struct_name : ", _index);
                    }
                }
            }
    })
}

fn parse_control_range(
    range: Expr,
    crate_ident: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    match range {
        Expr::Lit(lit) => {
            let textual_range = match lit.lit {
                syn::Lit::Str(lit_str) => match lit_str.value().to_lowercase().as_ref() {
                    "nyquist" => quote! { #crate_ident::ControlRange::Nyquist },
                    "infinity" => quote! { #crate_ident::ControlRange::Infinity },
                    "positiveinfinity" => {
                        quote! { #crate_ident::ControlRange::PositiveInfinity }
                    }
                    "negativeinfinity" => {
                        quote! { #crate_ident::ControlRange::NegativeInfinity }
                    }
                    _ => return Err(syn::Error::new(lit_str.span(), "Invalid ControlRange")),
                },
                _ => return Err(syn::Error::new(lit.span(), "Could not parse control range")),
            };
            Ok(textual_range)
        }
        Expr::Range(range) => {
            if range.start.is_none() {
                return Err(syn::Error::new(
                    range.span(),
                    "Control range must have a start value",
                ));
            } else if range.end.is_none() {
                return Err(syn::Error::new(
                    range.span(),
                    "Control range must have an end value",
                ));
            } else if let syn::RangeLimits::HalfOpen(_) = range.limits {
                return Err(syn::Error::new(
                    range.span(),
                    "Control range must be inclusive/closed",
                ));
            }
            let min = range.start.unwrap();
            let max = range.end.unwrap();
            // Ok(quote! { #crate_ident::ControlRange::range_inclusive(F::new(#min), F::new(#max)) })
            Ok(quote! { #crate_ident::ControlRange::RangeInclusive(F::new(#min), F::new(#max)) })
        }
        _ => Err(syn::Error::new(
            range.span(),
            "Control range syntax is invalid",
        )),
    }
}

trait PrimitiveToTokenStream {
    fn to_tokens(&self, crate_ident: &proc_macro2::TokenStream) -> proc_macro2::TokenStream;
}
impl PrimitiveToTokenStream for ControlPurpose {
    fn to_tokens(&self, crate_ident: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        match self {
            ControlPurpose::Frequency => quote!{ #crate_ident::ControlPurpose::Frequency },
            ControlPurpose::Phase => quote!{ #crate_ident::ControlPurpose::Phase },
            ControlPurpose::Q => quote!{ #crate_ident::ControlPurpose::Q },
            ControlPurpose::ReciprocalQ => quote!{ #crate_ident::ControlPurpose::ReciprocalQ },
            ControlPurpose::Amplitude => quote!{ #crate_ident::ControlPurpose::Amplitude },
            ControlPurpose::Offset => quote!{ #crate_ident::ControlPurpose::Offset },
            ControlPurpose::Seconds => quote!{ #crate_ident::ControlPurpose::Seconds },
            ControlPurpose::Pan => quote!{ #crate_ident::ControlPurpose::Pan },
            ControlPurpose::Decibel => quote!{ #crate_ident::ControlPurpose::Decibel },
        }
    }
}
