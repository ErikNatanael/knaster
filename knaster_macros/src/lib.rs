extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

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
            #[cfg(feature="std")]
        fn pinteger_descriptions(v: PInteger) -> ::std::string::String {
            ::std::format!("{:?}", #ident ::from(v))
        }
            #[cfg(feature="alloc")]
        fn pinteger_descriptions(v: PInteger) -> ::alloc::string::String {
            ::alloc::format!("{:?}", #ident ::from(v))
        }
    }
            };

    // Hand the output tokens back to the compiler
    TokenStream::from(expanded)
}
