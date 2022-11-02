use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::Error;

use super::parse_maybe_uninit;
use crate::util::{Attr, Named, Result};

pub(super) enum Arg {
    Param(Option<syn::token::Paren>, Attr<ParamArg>),
    Local(Option<syn::token::Paren>, Attr<LocalArg>),
    Global(Option<syn::token::Paren>, Attr<GlobalArg>),
    Simple(Option<syn::token::Paren>, Attr<SimpleArg>),
    Isotope(Option<syn::token::Paren>, Attr<IsotopeArg>),
    EntityCreator(Option<syn::token::Paren>, Attr<EntityCreatorArg>),
    EntityDeleter(Option<syn::token::Paren>, Attr<EntityDeleterArg>),
    EntityIterator(Option<syn::token::Paren>, Attr<EntityIteratorArg>),
}

impl Parse for Named<Arg> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "param" => parse_opt_list(input, Arg::Param)?,
            "local" => parse_opt_list(input, Arg::Local)?,
            "global" => parse_opt_list(input, Arg::Global)?,
            "simple" => parse_opt_list(input, Arg::Simple)?,
            "isotope" => parse_opt_list(input, Arg::Isotope)?,
            "entity_creator" => parse_opt_list(input, Arg::EntityCreator)?,
            "entity_deleter" => parse_opt_list(input, Arg::EntityDeleter)?,
            "entity_iterator" => parse_opt_list(input, Arg::EntityIterator)?,
            _ => return Err(Error::new_spanned(&name, "Unknown attribute")),
        };
        Ok(Named { name, value })
    }
}

fn parse_opt_list<T>(
    input: ParseStream,
    arg_opt_variant: fn(Option<syn::token::Paren>, Attr<T>) -> Arg,
) -> Result<Arg>
where
    Named<T>: Parse,
{
    let mut paren = None;
    let mut opts = Attr::default();

    if input.peek(syn::token::Paren) {
        let inner;
        paren = Some(syn::parenthesized!(inner in input));
        opts = inner.parse::<Attr<T>>()?;
    }

    Ok(arg_opt_variant(paren, opts))
}

pub(super) enum LocalArg {
    HasEntity,
    HasNoEntity,
    Initial(syn::Token![=], Box<syn::Expr>),
}

impl Parse for Named<LocalArg> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "entity" => LocalArg::HasEntity,
            "not_entity" => LocalArg::HasNoEntity,
            "initial" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let expr = input.parse::<syn::Expr>()?;
                LocalArg::Initial(eq, Box::new(expr))
            }
            _ => return Err(Error::new_spanned(&name, "Unknown option for #[dynec(local)]")),
        };
        Ok(Named { name, value })
    }
}

pub(super) enum ParamArg {
    HasEntity,
    HasNoEntity,
}

impl Parse for Named<ParamArg> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "entity" => ParamArg::HasEntity,
            "not_entity" => ParamArg::HasNoEntity,
            _ => return Err(Error::new_spanned(&name, "Unknown option for #[dynec(param)]")),
        };
        Ok(Named { name, value })
    }
}

pub(super) enum GlobalArg {
    ThreadLocal,
    MaybeUninit(syn::token::Paren, Punctuated<syn::Type, syn::Token![,]>),
}

impl Parse for Named<GlobalArg> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "thread_local" => GlobalArg::ThreadLocal,
            "maybe_uninit" => parse_maybe_uninit(input, GlobalArg::MaybeUninit)?,
            _ => return Err(Error::new_spanned(&name, "Unknown option for #[dynec(global)]")),
        };
        Ok(Named { name, value })
    }
}

pub(super) enum SimpleArg {
    Mutable,
    Arch(syn::Token![=], Box<syn::Type>),
    Comp(syn::Token![=], Box<syn::Type>),
    MaybeUninit(syn::token::Paren, Punctuated<syn::Type, syn::Token![,]>),
}

impl Parse for Named<SimpleArg> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "mut" => SimpleArg::Mutable,
            "arch" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                SimpleArg::Arch(eq, Box::new(ty))
            }
            "comp" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                SimpleArg::Comp(eq, Box::new(ty))
            }
            "maybe_uninit" => parse_maybe_uninit(input, SimpleArg::MaybeUninit)?,
            _ => return Err(Error::new_spanned(&name, "Unknown option for #[dynec(simple)]")),
        };
        Ok(Named { name, value })
    }
}

pub(super) enum IsotopeArg {
    Mutable,
    Arch(syn::Token![=], Box<syn::Type>),
    Comp(syn::Token![=], Box<syn::Type>),
    Discrim(syn::Token![=], Box<syn::Expr>),
    DiscrimKey(syn::Token![=], Box<syn::Type>),
    MaybeUninit(syn::token::Paren, Punctuated<syn::Type, syn::Token![,]>),
}

impl Parse for Named<IsotopeArg> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "mut" => IsotopeArg::Mutable,
            "arch" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                IsotopeArg::Arch(eq, Box::new(ty))
            }
            "comp" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                IsotopeArg::Comp(eq, Box::new(ty))
            }
            "discrim" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let discrim = input.parse::<syn::Expr>()?;
                IsotopeArg::Discrim(eq, Box::new(discrim))
            }
            "discrim_key" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                IsotopeArg::DiscrimKey(eq, Box::new(ty))
            }
            "maybe_uninit" => parse_maybe_uninit(input, IsotopeArg::MaybeUninit)?,
            _ => return Err(Error::new_spanned(&name, "Unknown option for #[dynec(isotope)]")),
        };
        Ok(Named { name, value })
    }
}

pub(super) enum EntityCreatorArg {
    Arch(syn::Token![=], Box<syn::Type>),
    NoPartition,
}

impl Parse for Named<EntityCreatorArg> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "arch" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                EntityCreatorArg::Arch(eq, Box::new(ty))
            }
            "no_partition" => EntityCreatorArg::NoPartition,
            _ => {
                return Err(Error::new_spanned(
                    &name,
                    "Unknown option for #[dynec(entity_creator)]",
                ))
            }
        };
        Ok(Named { name, value })
    }
}

pub(super) enum EntityDeleterArg {
    Arch(syn::Token![=], Box<syn::Type>),
}

impl Parse for Named<EntityDeleterArg> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "arch" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                EntityDeleterArg::Arch(eq, Box::new(ty))
            }
            _ => {
                return Err(Error::new_spanned(
                    &name,
                    "Unknown option for #[dynec(entity_deleter)]",
                ))
            }
        };
        Ok(Named { name, value })
    }
}

pub(super) enum EntityIteratorArg {
    Arch(syn::Token![=], Box<syn::Type>),
}

impl Parse for Named<EntityIteratorArg> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "arch" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                EntityIteratorArg::Arch(eq, Box::new(ty))
            }
            _ => {
                return Err(Error::new_spanned(
                    &name,
                    "Unknown option for #[dynec(entity_iterator)]",
                ))
            }
        };
        Ok(Named { name, value })
    }
}
