use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Error, Result};

/// A poorly optimized but stable implementation of `drain_filter`.
pub(crate) fn slow_drain_filter<T>(
    vec: &mut Vec<T>,
    mut filter: impl FnMut(&T) -> bool,
) -> impl IntoIterator<Item = T> {
    let mut i = 0;
    let mut output = Vec::new();

    while i < vec.len() {
        if !filter(vec.get(i).expect("just checked")) {
            i += 1;
            continue;
        }

        let item = vec.remove(i);
        output.push(item);
        // continue with same i since vec is updated
    }

    output
}

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
            #[automatically_derived]
            impl #decl #trait_ for #ident #usage #where_ {
                #body
            }
        }
    }
}

pub(crate) struct Attr<T> {
    pub(crate) items: Punctuated<Named<T>, syn::Token![,]>,
}

impl<T> Default for Attr<T> {
    fn default() -> Self { Self { items: Punctuated::new() } }
}

impl<T> Attr<T> {
    pub(crate) fn find_one<U>(&self, matcher: fn(&T) -> Option<&U>) -> Result<Option<(Span, &U)>> {
        let mut span: Option<(Span, &U)> = None;

        for item in &self.items {
            if let Some(t) = matcher(&item.value) {
                if let Some((prev, _)) = span {
                    return Err(Error::new(
                        prev.join(item.name.span()).unwrap_or(prev),
                        format!("only one `{}` argument is allowed", &item.name),
                    ));
                }

                span = Some((item.name.span(), t));
            }
        }

        Ok(span)
    }

    pub(crate) fn merge_all<'t, U, I: Iterator<Item = U> + 't>(
        &'t self,
        matcher: fn(&'t T) -> Option<I>,
    ) -> Vec<U> {
        let mut vec = Vec::new();

        for item in &self.items {
            vec.extend(matcher(&item.value).into_iter().flatten());
        }

        vec
    }
}

impl<T> Parse for Attr<T>
where
    Named<T>: Parse,
{
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Attr { items: Punctuated::parse_terminated(input)? })
    }
}

pub(crate) struct Named<T> {
    pub(crate) name:  syn::Ident,
    pub(crate) value: T,
}
