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

    if !args.is_empty() {
        let args: Attr<FnOpt> = syn::parse2(args)?;

        if let Some((_, value)) = args.find_one(|opt| match opt {
            FnOpt::Initial(value) => Some(value),
        })? {
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
        impl ::dynec::Global for #ident {
            #initial
        }
    };

    let mut mut_input = input;
    let entity_ref = entity_ref::entity_ref(&mut mut_input)?;

    Ok(quote! {
        #mut_input
        #global_impl
        #entity_ref
    })
}

enum FnOpt {
    Initial(Option<(syn::Token![=], syn::Expr)>),
}

impl Parse for Named<FnOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;

        let value = match name.to_string().as_str() {
            "initial" => {
                let value = if input.peek(syn::Token![=]) {
                    let eq: syn::Token![=] = input.parse()?;
                    let expr = input.parse::<syn::Expr>()?;
                    Some((eq, expr))
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
