use std::collections::HashMap;

use matches2::option_match;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::Error;

use crate::util::{Attr, Named, Result};

pub(crate) fn api(args: TokenStream, input: TokenStream) -> Result<TokenStream> {
    enum ApiOpt {
        DynecAs(syn::token::Paren, TokenStream),
        DebugPrint,
    }
    impl Parse for Named<ApiOpt> {
        fn parse(input: ParseStream) -> Result<Self> {
            let name = input.parse::<syn::Ident>()?;

            let value = match name.to_string().as_str() {
                "dynec_as" => {
                    let inner;
                    let paren = syn::parenthesized!(inner in input);
                    let args = inner.parse()?;
                    ApiOpt::DynecAs(paren, args)
                }
                "__debug_print" => ApiOpt::DebugPrint,
                _ => return Err(Error::new_spanned(&name, format!("Unknown argument `{}`", name))),
            };

            Ok(Named { name, value })
        }
    }

    let args: Attr<ApiOpt> = syn::parse2(args)?;

    let crate_name = if let Some((_, ts)) =
        args.find_one(|opt| option_match!(opt, ApiOpt::DynecAs(_, ts) => ts))?
    {
        ts.clone()
    } else {
        quote!(::dynec::)
    };

    let debug_print = args.find_one(|opt| option_match!(opt, ApiOpt::DebugPrint => &()))?.is_some();

    let output = quote! {
        #crate_name polyfill_tracer_decl!(#debug_print {#input});
    };
    Ok(output)
}

pub(crate) fn polyfill(input: TokenStream) -> Result<TokenStream> {
    let input: PolyfillInput = syn::parse2(input)?;

    let mut data: HashMap<_, _> =
        input.data.into_iter().map(|datum| (datum.to_key(), datum)).collect();

    for item in &input.impl_block.items {
        let key = match item {
            syn::ImplItem::Type(item) => PolyfillDataKey::Type(item.ident.clone()),
            syn::ImplItem::Fn(item) => PolyfillDataKey::Fn(item.sig.ident.clone()),
            _ => continue, // should cause compile error in later stages
        };
        data.remove(&key);
    }

    let mut impl_block = input.impl_block;

    for datum in data.into_values() {
        let ts = match datum {
            PolyfillData::Type(_, ident) => quote! {
                type #ident = ();
            },
            PolyfillData::Fn(_, _, body) => body,
        };
        let ts = syn::parse2::<syn::ImplItem>(ts).expect("invalid impl item");
        impl_block.items.push(ts);
    }

    let output = quote! {
        #impl_block
    };
    if input.debug_print.value {
        println!("output: {output}");
    }
    Ok(output)
}

struct PolyfillInput {
    _crate_name: TokenStream,
    debug_print: syn::LitBool,
    impl_block:  syn::ItemImpl,
    data:        Vec<PolyfillData>,
}

impl Parse for PolyfillInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let crate_name = {
            let inner;
            _ = syn::braced!(inner in input);
            inner.parse()?
        };
        let debug_print = input.parse()?;
        let impl_block = {
            let inner;
            _ = syn::braced!(inner in input);
            inner.parse()?
        };

        let mut data = Vec::new();
        while !input.is_empty() {
            let token = input.lookahead1();
            let datum = if token.peek(syn::Token![type]) {
                PolyfillData::Type(input.parse()?, input.parse()?)
            } else if token.peek(syn::Token![fn]) {
                let fn_token = input.parse()?;
                let ident = input.parse()?;
                let inner;
                _ = syn::braced!(inner in input);
                let filler = inner.parse()?;
                PolyfillData::Fn(fn_token, ident, filler)
            } else {
                return Err(token.error());
            };
            data.push(datum);
        }

        Ok(Self { _crate_name: crate_name, debug_print, impl_block, data })
    }
}

enum PolyfillData {
    Type(syn::Token![type], proc_macro2::Ident),
    Fn(syn::Token![fn], proc_macro2::Ident, TokenStream),
}

impl PolyfillData {
    fn to_key(&self) -> PolyfillDataKey {
        match self {
            Self::Type(_, ident) => PolyfillDataKey::Type(ident.clone()),
            Self::Fn(_, ident, _) => PolyfillDataKey::Fn(ident.clone()),
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
enum PolyfillDataKey {
    Type(proc_macro2::Ident),
    Fn(proc_macro2::Ident),
}
