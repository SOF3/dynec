use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;

use crate::util::Result;

pub(crate) fn imp(input: TokenStream) -> Result<TokenStream> {
    let Input { dynec_as, archetype, components, .. } = syn::parse2(input)?;

    let crate_name = match dynec_as {
        Some(ts) => ts,
        None => quote!(::dynec),
    };

    let comp_map = quote!(__dynec_map);

    let components = components.iter().map(|component| {
        let expr = &component.expr;

        let item = match component.iso {
            None => quote_spanned! { expr.span() =>
                |expr| #comp_map.insert_simple(expr)
            },
            Some(iso) => quote_spanned! { iso.span() =>
                |(discrim, value)| {
                    #comp_map.insert_isotope(discrim, value);
                }
            },
        };

        let iter_expr = match component.iter {
            None => quote_spanned!(expr.span() => [#expr]),
            Some(iter) => quote_spanned!(iter.span() => #expr),
        };

        quote_spanned! { expr.span() =>
            (#iter_expr).into_iter().for_each(#item);
        }
    });

    let output = quote! {{
        let mut #comp_map = #crate_name::comp::Map::<#archetype>::default();
        #(#components)*
        #comp_map
    }};

    Ok(output)
}

struct Input {
    dynec_as:   Option<TokenStream>,
    archetype:  syn::Type,
    _arrow:     syn::Token![=>],
    components: Punctuated<Component, syn::Token![,]>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut dynec_as = None;

        while input.peek(syn::Token![@]) {
            input.parse::<syn::Token![@]>()?;
            let inner;
            syn::parenthesized!(inner in input);
            let args = inner.parse()?;
            dynec_as = Some(args);
        }

        let archetype = input.parse()?;
        let arrow = input.parse()?;
        let components = Punctuated::parse_terminated(input)?;

        Ok(Self { dynec_as, archetype, _arrow: arrow, components })
    }
}

struct Component {
    iso:  Option<syn::Token![@]>,
    iter: Option<syn::Token![?]>,
    expr: syn::Expr,
}

impl Parse for Component {
    fn parse(input: ParseStream) -> Result<Self> {
        let iso = if input.peek(syn::Token![@]) {
            let token: syn::Token![@] = input.parse()?;
            Some(token)
        } else {
            None
        };

        let iter = if input.peek(syn::Token![?]) {
            let token: syn::Token![?] = input.parse()?;
            Some(token)
        } else {
            None
        };

        let expr: syn::Expr = input.parse()?;

        Ok(Self { iso, iter, expr })
    }
}
