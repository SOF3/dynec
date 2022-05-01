use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::Result;

pub(crate) fn imp(input: TokenStream) -> Result<TokenStream> {
    let Input { dynec_as, archetype, components, .. } = syn::parse2(input)?;

    let crate_name = match dynec_as {
        Some(ts) => ts,
        None => quote!(::dynec),
    };

    let components = components.iter().map(|component| match component {
        Component::Simple(expr) => quote! {
            __dynec_map.insert_simple(#expr);
        },
        Component::Isotope(_, expr) => quote! {
            for (discrim, value) in #expr {
                __dynec_map.insert_isotope(discrim, value);
            }
        },
    });

    let output = quote! {{
        let mut __dynec_map = #crate_name::component::Map::<#archetype>::default();
        #(#components)*
        __dynec_map
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

enum Component {
    Simple(syn::Expr),
    Isotope(syn::Token![@], syn::Expr),
}

impl Parse for Component {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(syn::Token![@]) {
            let at = input.parse::<syn::Token![@]>()?;
            let expr = syn::Expr::parse(input)?;
            Ok(Self::Isotope(at, expr))
        } else {
            let expr = syn::Expr::parse(input)?;
            Ok(Self::Simple(expr))
        }
    }
}
