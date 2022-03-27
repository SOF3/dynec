use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Error, Result};

use crate::{has_ref, util};

pub(crate) fn imp(args: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let args: Args = syn::parse2(args)?;

    let archetypes: Vec<_> = args
        .args
        .iter()
        .filter_map(|arg| match &arg.value {
            Arg::Of(_, ty) => Some(ty),
            _ => None,
        })
        .collect();

    let isotope = args.find_one(|arg| match arg {
        Arg::Isotope(_, discrim) => Some(discrim),
        _ => None,
    })?;

    let presence = args.find_one(|arg| match arg {
        Arg::Required => Some(&()),
        _ => None,
    })?;
    if let (Some((isotope_span, _)), Some((presence_span, _))) = (isotope, presence) {
        return Err(Error::new(
            isotope_span.join(presence_span).unwrap_or(presence_span),
            "isotope components cannot be required because new isotopes may be created dynamically",
        ));
    }
    let presence = match presence {
        Some(_) => quote!(::dynec::component::SimplePresence::Required),
        None => quote!(::dynec::component::SimplePresence::Optional),
    };

    let finalizer = args.find_one(|arg| match arg {
        Arg::Finalizer => Some(&()),
        _ => None,
    })?;
    if let (Some((isotope_span, _)), Some((finalizer_span, _))) = (isotope, finalizer) {
        return Err(Error::new(
            isotope_span.join(finalizer_span).unwrap_or(finalizer_span),
            "isotope components cannot be finalizers",
        ));
    }
    let finalizer = finalizer.is_some();

    let init = args.find_one(|arg| match arg {
        Arg::Init(_, func) => Some(func),
        _ => None,
    })?;

    let input: syn::DeriveInput = syn::parse2(input)?;
    let generics = util::parse_generics(&input);

    let mut output = TokenStream::new();
    for archetype in archetypes {
        if let Some((_, discrim)) = isotope {
            let init = match init {
                Some((_, func)) => quote!({
                    ::dynec::component::IsotopeInitStrategy::Default(#func)
                }),
                None => quote!(::dynec::component::IsotopeInitStrategy::None),
            };

            output.extend(generics.impl_trait(
                quote!(::dynec::component::Isotope<#archetype>),
                quote! {
                    type Discrim = #discrim;

                    const INIT_STRATEGY: ::dynec::component::IsotopeInitStrategy<Self> = #init;
                },
            ));
        } else {
            let init = match init {
                Some((_, func)) => quote!({
                    ::dynec::component::SimpleInitStrategy::auto(&(#func))
                }),
                None => quote!(::dynec::component::SimpleInitStrategy::None),
            };

            output.extend(generics.impl_trait(quote!(::dynec::component::Simple<#archetype>), quote! {
                const PRESENCE: ::dynec::component::SimplePresence = #presence;
                const INIT_STRATEGY: ::dynec::component::SimpleInitStrategy<#archetype, Self> = #init;
                const IS_FINALIZER: bool = #finalizer;
            }));
        }
    }

    let mut mut_input = input;
    let has_ref = has_ref::has_ref(&mut mut_input)?;

    Ok(quote! {
        #mut_input
        #output
        #has_ref
    })
}

struct Args {
    args: Punctuated<NamedArg, syn::Token![,]>,
}

impl Args {
    fn find_one<T>(&self, matcher: fn(&Arg) -> Option<&T>) -> Result<Option<(Span, &T)>> {
        let mut span: Option<(Span, &T)> = None;

        for arg in &self.args {
            if let Some(t) = matcher(&arg.value) {
                if let Some((prev, _)) = span {
                    return Err(Error::new(
                        prev.join(arg.name.span()).unwrap_or(prev),
                        format!("only one `{}` argument is allowed", &arg.name),
                    ));
                }

                span = Some((arg.name.span(), t));
            }
        }

        Ok(span)
    }
}

impl Parse for Args {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Args { args: Punctuated::parse_separated_nonempty(input)? })
    }
}

struct NamedArg {
    name:  syn::Ident,
    value: Arg,
}

enum Arg {
    Of(syn::Token![=], syn::Type),
    Isotope(syn::Token![=], syn::Type),
    Required,
    Finalizer,
    Init(syn::Token![=], Box<syn::Expr>),
}

impl Parse for NamedArg {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;

        let value = match name.to_string().as_str() {
            "of" => {
                let eq: syn::Token![=] = input.parse()?;
                let ty = input.parse::<syn::Type>()?;
                Arg::Of(eq, ty)
            }
            "isotope" => {
                let eq: syn::Token![=] = input.parse()?;
                let ty = input.parse::<syn::Type>()?;
                Arg::Isotope(eq, ty)
            }
            "required" => Arg::Required,
            "finalizer" => Arg::Finalizer,
            "init" => {
                let eq: syn::Token![=] = input.parse()?;
                let expr = input.parse::<syn::Expr>()?;
                Arg::Init(eq, Box::new(expr))
            }
            _ => return Err(Error::new_spanned(&name, format!("Unknown argument `{}`", name))),
        };

        Ok(NamedArg { name, value })
    }
}
