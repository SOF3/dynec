use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Error, Result,
};

use crate::util;

pub(crate) fn imp(input: TokenStream) -> Result<TokenStream> {
    let input: syn::DeriveInput = syn::parse2(input)?;

    let generics = util::parse_generics(&input);

    let mut factory = None;
    let mut archetypes = Vec::new();
    let mut multi_ord = None;

    for attr in &input.attrs {
        if attr.path.is_ident("component") {
            let attr: Attr = attr.parse_args()?;
            for opt in attr.options {
                match opt {
                    Opt::Required(name) | Opt::Optional(name) | Opt::Default(name, _)
                        if factory.is_some() =>
                    {
                        return Err(Error::new_spanned(
                            name,
                            "Only one of `required`, `optional` and `default` is allowed",
                        ));
                    }
                    Opt::Required(..) => factory = Some(Factory::Required),
                    Opt::Optional(..) => factory = Some(Factory::Optional),
                    Opt::Default(_, Some((_, expr))) => {
                        factory = Some(Factory::Default(expr.to_token_stream()))
                    }
                    Opt::Default(_, None) => {
                        factory = Some(Factory::Default(quote!(Default::default())))
                    }
                    Opt::Of(_, _, ty) => archetypes.push(ty),
                    Opt::Multi(name, _, _) if multi_ord.is_some() => {
                        return Err(Error::new_spanned(
                            name,
                            "`multi` can only be specified once",
                        ));
                    }
                    Opt::Multi(_, _, ty) => multi_ord = Some(ty),
                }
            }
        }
    }

    let factory = match factory {
        Some(factory) => factory,
        None => {
            return Err(Error::new(
                Span::call_site(),
                "One of `required`, `optional` or `default` must be specified",
            ))
        }
    };

    match &input.data {
        syn::Data::Struct(_data) => {}
        syn::Data::Enum(_data) => {}
        _ => {
            return Err(Error::new(
                Span::call_site(),
                "Component can only be derived from structs or enums",
            ))
        }
    }

    let single_must =
        if multi_ord.is_none() && matches!(factory, Factory::Required | Factory::Default(_)) {
            generics.impl_trait(quote!(::dynec::component::SingleMust), quote!())
        } else {
            quote!()
        };

    let factory_expr = match factory {
        Factory::Required => quote!(::dynec::component::Factory::RequiredInput),
        Factory::Optional => quote!(::dynec::component::Factory::Optional),
        Factory::Default(expr) => quote! {
            ::dynec::component::Factory::AutoInit(Box::new(|| #expr))
        },
    };

    let impl_comp = generics.impl_trait(
        quote!(::dynec::Component),
        quote! {
            fn as_any(&self) -> &dyn ::std::any::Any { self }

            fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any { self }

            fn factory() -> ::dynec::component::Factory<Self> { #factory_expr }
        },
    );

    let single_multi = match multi_ord {
        Some(ord_ty) => generics.impl_trait(
            quote!(::dynec::component::Multi),
            quote! {
                type Ord = #ord_ty;
            },
        ),
        None => generics.impl_trait(quote!(::dynec::component::Single), quote!()),
    };

    let contains = archetypes.iter().map(|archetype| {
        generics.impl_trait_with(
            |self_| quote!(::dynec::archetype::Contains<#self_>),
            archetype.to_token_stream(),
            quote!(),
        )
    });

    Ok(quote! {
        #[automatically_derived]
        #impl_comp

        #single_multi
        #single_must

        #(
            #[automatically_derived]
            #contains
        )*
    })
}

enum Factory {
    Required,
    Optional,
    Default(TokenStream),
}

struct Attr {
    options: Punctuated<Opt, syn::Token![,]>,
}

impl Parse for Attr {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Attr {
            options: Punctuated::parse_separated_nonempty(input)?,
        })
    }
}

enum Opt {
    Of(syn::Ident, syn::Token![=], syn::Type),
    Multi(syn::Ident, syn::Token![=], syn::Type),
    Required(syn::Ident),
    Optional(syn::Ident),
    Default(syn::Ident, Option<(syn::Token![=], syn::Expr)>),
}

impl Parse for Opt {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;

        let attr = match name.to_string().as_str() {
            "of" => Opt::Of(
                name,
                input.parse::<syn::Token![=]>()?,
                input.parse::<syn::Type>()?,
            ),
            "multi" => Opt::Multi(
                name,
                input.parse::<syn::Token![=]>()?,
                input.parse::<syn::Type>()?,
            ),
            "required" => Opt::Required(name),
            "optional" => Opt::Optional(name),
            "default" => Opt::Default(name, {
                if input.peek(syn::Token![=]) {
                    Some((
                        input.parse::<syn::Token![=]>()?,
                        input.parse::<syn::Expr>()?,
                    ))
                } else {
                    None
                }
            }),
            _ => return Err(Error::new_spanned(&name, "Unknown attribute")),
        };

        Ok(attr)
    }
}
