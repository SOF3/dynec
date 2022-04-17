use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::Result;

pub(crate) fn imp(input: TokenStream) -> Result<TokenStream> {
    let inputs: Inputs = syn::parse2(input)?;
    Ok(inputs
        .0
        .into_iter()
        .map(|Input { meta, vis, ident, .. }| {
            quote! {
                #(#meta)*
                #vis enum #ident {}

                impl ::dynec::Archetype for #ident {}
            }
        })
        .collect())
}

struct Inputs(Vec<Input>);

impl Parse for Inputs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut inputs = Vec::new();
        while !input.is_empty() {
            inputs.push(input.parse()?);
        }
        Ok(Self(inputs))
    }
}

struct Input {
    meta:  Vec<syn::Attribute>,
    vis:   syn::Visibility,
    ident: syn::Ident,
    _semi: Option<syn::Token![;]>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> Result<Self> {
        let meta = input.call(syn::Attribute::parse_outer)?;
        let vis = input.parse()?;
        let ident = input.parse()?;
        let semi = if input.peek(syn::Token![;]) { Some(input.parse()?) } else { None };

        Ok(Self { meta, vis, ident, _semi: semi })
    }
}
