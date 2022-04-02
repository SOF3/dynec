use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;

pub(crate) fn parse_generics(input: &syn::DeriveInput) -> ParsedGenerics {
    let generics = &input.generics;

    let (decl, usage) = if input.generics.params.is_empty() {
        (quote!(), quote!())
    } else {
        let decl: Vec<_> = input.generics.params.iter().collect();
        let usage: Vec<_> = input
            .generics
            .params
            .iter()
            .map(|param| match param {
                syn::GenericParam::Type(syn::TypeParam { ident, .. }) => quote!(#ident),
                syn::GenericParam::Lifetime(syn::LifetimeDef { lifetime, .. }) => {
                    quote!(#lifetime)
                }
                syn::GenericParam::Const(syn::ConstParam { ident, .. }) => quote!(#ident),
            })
            .collect();
        (
            quote_spanned!(generics.span() => <#(#decl),*>),
            quote_spanned!(generics.span() => <#(#usage),*>),
        )
    };

    let where_ = &input.generics.where_clause;

    ParsedGenerics { ident: input.ident.clone(), decl, usage, where_: where_.to_token_stream() }
}

pub(crate) struct ParsedGenerics {
    pub(crate) ident:  proc_macro2::Ident,
    pub(crate) decl:   proc_macro2::TokenStream,
    pub(crate) usage:  proc_macro2::TokenStream,
    pub(crate) where_: proc_macro2::TokenStream,
}

impl ParsedGenerics {
    pub(crate) fn impl_trait(
        &self,
        trait_: TokenStream,
        body: TokenStream,
    ) -> proc_macro2::TokenStream {
        let Self { ident, decl, usage, where_ } = self;
        quote! {
            impl #decl #trait_ for #ident #usage #where_ {
                #body
            }
        }
    }

    pub(crate) fn impl_trait_with(
        &self,
        trait_: impl FnOnce(TokenStream) -> TokenStream,
        on: TokenStream,
        body: TokenStream,
    ) -> proc_macro2::TokenStream {
        let Self { ident, decl, usage, where_ } = self;
        let trait_ = trait_(quote!(#ident #usage));
        quote! {
            impl #decl #trait_ for #on #where_ {
                #body
            }
        }
    }
}
