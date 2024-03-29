use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::Error;

pub(crate) type Result<T, E = syn::Error> = std::result::Result<T, E>;

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
                syn::GenericParam::Lifetime(syn::LifetimeParam { lifetime, .. }) => {
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

pub(crate) struct Attr<T, P = syn::Token![,]> {
    pub(crate) items: Punctuated<Named<T>, P>,
}

impl<T, P> Default for Attr<T, P> {
    fn default() -> Self { Self { items: Punctuated::new() } }
}

impl<T, P> Attr<T, P> {
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

impl<T, P> FromIterator<Attr<T, P>> for Attr<T, ()> {
    fn from_iter<I: IntoIterator<Item = Attr<T, P>>>(iter: I) -> Self {
        let mut items = Punctuated::new();
        for group in iter {
            items.extend(group.items);
        }
        Attr { items }
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

pub(crate) fn parse_attrs<T>(attrs: &mut Vec<syn::Attribute>) -> syn::Result<Attr<T, ()>>
where
    Named<T>: Parse,
{
    attrs
        .extract_if(|attr| attr.path().is_ident("dynec"))
        .map(|attr| attr.parse_args::<Attr<T>>())
        .collect()
}
