use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::Result;

pub(crate) fn imp(input: TokenStream) -> Result<TokenStream> {
    let Input { meta, vis, ident } = syn::parse2(input)?;

    Ok(quote! {
        #(#meta)*
        #vis enum #ident {}

        impl ::dynec::Archetype for #ident {}
    })
}

struct Input {
    meta:  Vec<syn::Attribute>,
    vis:   syn::Visibility,
    ident: syn::Ident,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> Result<Self> {
        let meta = input.call(syn::Attribute::parse_outer)?;
        let vis = input.parse()?;
        let ident = input.parse()?;

        Ok(Self { meta, vis, ident })
    }
}
