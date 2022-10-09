use matches2::option_match;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::Error;

use super::parse_maybe_uninit;
use crate::util::{Attr, Named, Result};

pub(super) enum Opt {
    DynecAs(syn::token::Paren, TokenStream),
    ThreadLocal,
    DebugPrint, /* introduced due to high frequency of need to debug this macro and its enormous output */
    Before(syn::token::Paren, Punctuated<syn::Expr, syn::Token![,]>),
    After(syn::token::Paren, Punctuated<syn::Expr, syn::Token![,]>),
    Name(syn::Token![=], Box<syn::Expr>),
    MaybeUninit(syn::token::Paren, Punctuated<syn::Type, syn::Token![,]>),
}

impl Parse for Named<Opt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "dynec_as" => {
                let inner;
                let paren = syn::parenthesized!(inner in input);
                let args = inner.parse()?;
                Opt::DynecAs(paren, args)
            }
            "thread_local" => Opt::ThreadLocal,
            "__debug_print" => Opt::DebugPrint,
            "before" => {
                let inner;
                let paren = syn::parenthesized!(inner in input);
                Opt::Before(paren, Punctuated::parse_terminated(&inner)?)
            }
            "after" => {
                let inner;
                let paren = syn::parenthesized!(inner in input);
                Opt::After(paren, Punctuated::parse_terminated(&inner)?)
            }
            "name" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let name = input.parse::<syn::Expr>()?;
                Opt::Name(eq, Box::new(name))
            }
            "maybe_uninit" => parse_maybe_uninit(input, Opt::MaybeUninit)?,
            _ => return Err(Error::new_spanned(&name, "Unknown attribute")),
        };

        Ok(Named { name, value })
    }
}

pub(super) struct Agg {
    pub(super) crate_name:          TokenStream,
    pub(super) system_thread_local: bool,
    pub(super) debug_print:         bool,
    pub(super) state_maybe_uninit:  Vec<syn::Type>,
    pub(super) name:                TokenStream,
    pub(super) deps:                Vec<TokenStream>,
}

impl Agg {
    pub(super) fn parse(ident: &syn::Ident, args: TokenStream) -> Result<Self> {
        let mut agg = Agg {
            crate_name:          quote!(::dynec),
            system_thread_local: false,
            debug_print:         false,
            state_maybe_uninit:  Vec::new(),
            name:                quote!(concat!(module_path!(), "::", stringify!(#ident))),
            deps:                Vec::new(),
        };

        if args.is_empty() {
            return Ok(agg);
        }

        let args = syn::parse2::<Attr<Opt>>(args)?;

        if let Some((_, ts)) = args.find_one(|opt| option_match!(opt, Opt::DynecAs(_, ts) => ts))? {
            agg.crate_name = ts.clone();
        }

        agg.system_thread_local =
            args.find_one(|opt| option_match!(opt, Opt::ThreadLocal => &()))?.is_some();
        agg.debug_print =
            args.find_one(|opt| option_match!(opt, Opt::DebugPrint => &()))?.is_some();
        agg.state_maybe_uninit = args.merge_all(
            |opt| option_match!(opt, Opt::MaybeUninit(_, archs) => archs.iter().cloned()),
        );

        let crate_name = &agg.crate_name;
        for named in &args.items {
            match &named.value {
                Opt::DynecAs(_, _) => {} // already handled
                Opt::ThreadLocal => {}   // already handled
                Opt::DebugPrint => {}    // already handled
                Opt::Before(_, inputs) => {
                    for dep in inputs {
                        agg.deps.push(quote!(#crate_name::system::spec::Dependency::Before(Box::new(#dep) as Box<dyn #crate_name::system::Partition>)));
                    }
                }
                Opt::After(_, inputs) => {
                    for dep in inputs {
                        agg.deps.push(quote!(#crate_name::system::spec::Dependency::After(Box::new(#dep) as Box<dyn #crate_name::system::Partition>)));
                    }
                }
                Opt::Name(_, name_expr) => {
                    agg.name = quote!(#name_expr);
                }
                Opt::MaybeUninit(_, _) => {} // already handled
            }
        }

        Ok(agg)
    }
}
