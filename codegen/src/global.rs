use matches2::option_match;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Error, Result};

use crate::entity_ref;
use crate::util::{Attr, Named};

pub(crate) fn imp(args: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let mut initial = None;

    let input: syn::DeriveInput = syn::parse2(input)?;
    let ident = &input.ident;

    let mut crate_name = quote!(::dynec);

    if !args.is_empty() {
        let args: Attr<FnOpt> = syn::parse2(args)?;

        if let Some((_, ts)) =
            args.find_one(|opt| option_match!(opt, FnOpt::DynecAs(_, ts) => ts))?
        {
            crate_name = ts.clone();
        }

        if let Some((_, value)) =
            args.find_one(|opt| option_match!(opt, FnOpt::Initial(value) => value))?
        {
            let value = match value {
                Some((_, value)) => quote!(#value),
                None => quote!(::std::default::Default::default()),
            };

            initial = Some(quote! {
                fn initial() -> Self {
                    #value
                }
            });
        }
    }

    let global_impl = quote! {
        impl #crate_name::Global for #ident {
            #initial
        }
    };

    let mut mut_input = input;
    let entity_ref = entity_ref::entity_ref(
        &mut mut_input,
        crate_name,
        quote! {
            this_field_references_an_entity_so_it_should_have_the_entity_attribute
        },
    )?;

    Ok(quote! {
        #mut_input
        #global_impl
        #entity_ref
    })
}

enum FnOpt {
    DynecAs(syn::token::Paren, TokenStream),
    Initial(Option<(syn::Token![=], Box<syn::Expr>)>),
}

impl Parse for Named<FnOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;

        let value = match name.to_string().as_str() {
            "dynec_as" => {
                let inner;
                let paren = syn::parenthesized!(inner in input);
                let args = inner.parse()?;
                FnOpt::DynecAs(paren, args)
            }
            "initial" => {
                let value = if input.peek(syn::Token![=]) {
                    let eq: syn::Token![=] = input.parse()?;
                    let expr = input.parse::<syn::Expr>()?;
                    Some((eq, Box::new(expr)))
                } else {
                    None
                };
                FnOpt::Initial(value)
            }
            _ => return Err(Error::new_spanned(&name, format!("Unknown argument `{}`", name))),
        };

        Ok(Named { name, value })
    }
}
