use matches2::option_match;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::Error;

use crate::util::{Attr, Named, Result};

pub(crate) fn imp(input: TokenStream) -> Result<TokenStream> {
    let inputs: Inputs = syn::parse2(input)?;

    let mut output = TokenStream::new();

    for Input { crate_name, meta, vis, ident, opts, .. } in inputs.0 {
        let crate_name = match crate_name {
            Some((_, crate_name)) => crate_name,
            None => quote!(::dynec),
        };

        let raw_entity =
            match opts.find_one(|opt| option_match!(opt, Opt::RawEntity(_, ty) => ty))? {
                Some((_, ty)) => ty.into_token_stream(),
                None => quote!(::std::num::NonZeroU32),
            };

        let recycler = match opts.find_one(|opt| option_match!(opt, Opt::Recycler(_, ty) => ty))? {
            Some((_, ty)) => ty.into_token_stream(),
            None => quote!(::std::collections::BTreeSet<#raw_entity>),
        };

        let shard_assigner =
            match opts.find_one(|opt| option_match!(opt, Opt::ShardAssigner(_, ty) => ty))? {
                Some((_, ty)) => ty.into_token_stream(),
                None => quote!(#crate_name::entity::ealloc::ThreadRngShardAssigner),
            };

        let block_size =
            match opts.find_one(|opt| option_match!(opt, Opt::BlockSize(_, expr) => expr))? {
                Some((_, ty)) => ty.into_token_stream(),
                None => quote!(16), // 16 * 32bits = 64 bytes
            };

        let item = quote! {
            #(#meta)*
            #vis enum #ident {}

            impl #crate_name::Archetype for #ident {
                type RawEntity = #raw_entity;
                type Ealloc = #crate_name::entity::ealloc::Recycling<#raw_entity, #recycler, #shard_assigner, #block_size>;
            }
        };
        output.extend(item);
    }

    Ok(output)
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
    crate_name: Option<(syn::Token![@], TokenStream)>,
    meta:       Vec<syn::Attribute>,
    vis:        syn::Visibility,
    ident:      syn::Ident,
    opts:       Attr<Opt>,
    _semi:      Option<syn::Token![;]>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> Result<Self> {
        let crate_name = if input.peek(syn::Token![@]) {
            let at = input.parse()?;
            let crate_name = input.parse()?;
            Some((at, crate_name))
        } else {
            None
        };

        let meta = input.call(syn::Attribute::parse_outer)?;
        let vis = input.parse()?;
        let ident = input.parse()?;
        let opts = if input.peek(syn::token::Paren) {
            let inner;
            syn::parenthesized!(inner in input);
            inner.parse::<Attr<Opt>>()?
        } else {
            Attr::default()
        };
        let semi = if input.peek(syn::Token![;]) { Some(input.parse()?) } else { None };

        Ok(Self { crate_name, meta, vis, ident, opts, _semi: semi })
    }
}

enum Opt {
    RawEntity(syn::Token![=], syn::Type),
    Recycler(syn::Token![=], syn::Type),
    ShardAssigner(syn::Token![=], syn::Type),
    BlockSize(syn::Token![=], syn::Expr),
}

impl Parse for Named<Opt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;

        let value = match name.to_string().as_str() {
            "raw_entity" => {
                let eq: syn::Token![=] = input.parse()?;
                let ty = input.parse::<syn::Type>()?;
                Opt::RawEntity(eq, ty)
            }
            "recycler" => {
                let eq: syn::Token![=] = input.parse()?;
                let ty = input.parse::<syn::Type>()?;
                Opt::Recycler(eq, ty)
            }
            "shard_assigner" => {
                let eq: syn::Token![=] = input.parse()?;
                let ty = input.parse::<syn::Type>()?;
                Opt::ShardAssigner(eq, ty)
            }
            "block_size" => {
                let eq: syn::Token![=] = input.parse()?;
                let expr = input.parse::<syn::Expr>()?;
                Opt::BlockSize(eq, expr)
            }
            _ => return Err(Error::new_spanned(&name, format!("Unknown argument `{}`", name))),
        };

        Ok(Named { name, value })
    }
}
