use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Error, Result};

use crate::util::{Attr, Named};

pub(crate) fn imp(args: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let input: syn::ItemFn = syn::parse2(input)?;
    let ident = &input.sig.ident;
    let vis = &input.vis;
    let other_attrs = &input.attrs;

    let mut deps = Vec::new();

    let mut name = quote!(concat!(module_path!(), "::", stringify!(#ident)));

    if !args.is_empty() {
        let args = syn::parse2::<Attr<FnOpt>>(args)?;

        for named in &args.items {
            match &named.value {
                FnOpt::Before(_, inputs) => {
                    for dep in inputs {
                        deps.push(quote!(::dynec::system::spec::Dependency::Before(Box::new(#dep as Box<dyn ::dynec::system::Partition>))));
                    }
                }
                FnOpt::After(_, inputs) => {
                    for dep in inputs {
                        deps.push(quote!(::dynec::system::spec::Dependency::After(Box::new(#dep as Box<dyn ::dynec::system::Partition>))));
                    }
                }
                FnOpt::Name(_, name_expr) => {
                    name = quote!(#name_expr);
                }
            }
        }
    }

    if !matches!(input.sig.output, syn::ReturnType::Default) {
        return Err(Error::new_spanned(&input.sig.output, "system functions must return unit"));
    }

    let mut local_fields = Vec::new();

    for (param_index, param) in input.sig.inputs.iter().enumerate() {
        let param = match param {
            syn::FnArg::Receiver(receiver) => {
                return Err(Error::new_spanned(receiver, "system funcions must not be a method"))
            }
            syn::FnArg::Typed(typed) => typed,
        };

        enum ArgType {
            Local { default: Option<Box<syn::Expr>> },
            Global,
        }

        let mut arg_type: Option<Named<ArgType>> = None;

        fn set_arg_type(
            arg_type: &mut Option<Named<ArgType>>,
            ident: syn::Ident,
            ty: ArgType,
        ) -> Result<()> {
            if let Some(no) = &arg_type {
                return Err(Error::new(
                    no.name.span().join(ident.span()).unwrap_or_else(|| no.name.span()),
                    "Each argument can only have one argument type",
                ));
            }

            *arg_type = Some(Named { name: ident, value: ty });

            Ok(())
        }

        for attr in &param.attrs {
            if attr.path.is_ident("dynec") {
                let arg_attr = attr.parse_args::<Attr<ArgOpt>>()?;

                for arg in arg_attr.items {
                    match arg.value {
                        ArgOpt::Param => {
                            set_arg_type(
                                &mut arg_type,
                                arg.name,
                                ArgType::Local { default: None },
                            )?;
                        }
                        ArgOpt::Local(_, default) => {
                            set_arg_type(
                                &mut arg_type,
                                arg.name,
                                ArgType::Local { default: Some(default) },
                            )?;
                        }
                        ArgOpt::Global => {
                            set_arg_type(&mut arg_type, arg.name, ArgType::Global)?;
                        }
                    }
                }
            }
        }

        let arg_type = match arg_type {
            Some(arg_type) => arg_type,
            None => {
                todo!()
            }
        };

        match arg_type.value {
            ArgType::Local { default } => {
                let field_name = match &*param.pat {
                    syn::Pat::Ident(ident) => ident.ident.clone(),
                    _ => quote::format_ident!("__dynec_arg_{}", param_index),
                };

                let param_ty = &param.ty;

                local_fields.push(quote! {
                    #field_name: #param_ty,
                });
            }
            _ => todo!(),
        }
    }

    let fn_body = &*input.block;

    Ok(quote! {
        #(#[#other_attrs])*
        #[allow(non_camel_case_types)]
        #vis struct #ident {
            #(#local_fields,)*
        }

        const _: () = {
            impl ::dynec::system::Spec for #ident {
                fn debug_name(&self) -> String {
                    String::from(#name)
                }
            }
        };
    })
}

fn extract_bound(bound: &syn::TypeParamBound) -> Result<&syn::PathSegment> {
    let bound = match bound {
        syn::TypeParamBound::Trait(bound) => bound,
        _ => {
            return Err(Error::new_spanned(
                bound,
                "Lifetimes are not allowed in system function signature",
            ))
        }
    };

    if !matches!(bound.modifier, syn::TraitBoundModifier::None) {
        return Err(Error::new_spanned(
            &bound.modifier,
            "The `?` modifier is not allowed in system function signature",
        ));
    }

    if bound.lifetimes.is_some() {
        return Err(Error::new_spanned(
            &bound.modifier,
            "Higher-rank trait bounds are not allowed in system function signature",
        ));
    }

    let last = bound.path.segments.last().expect("Path segments cannot be empty");

    Ok(last)
}

fn extract_type<'t>(
    args: &'t syn::PathArguments,
    index: usize,
    err: &str,
) -> Result<&'t syn::Type> {
    let args = match args {
        syn::PathArguments::AngleBracketed(args) => &args.args,
        _ => return Err(Error::new_spanned(&args, err)),
    };

    let ty = match args.iter().nth(index) {
        Some(syn::GenericArgument::Type(ty)) => ty,
        _ => return Err(Error::new_spanned(&args, err)),
    };

    Ok(ty)
}

enum FnOpt {
    Before(syn::token::Paren, Punctuated<syn::Expr, syn::Token![,]>),
    After(syn::token::Paren, Punctuated<syn::Expr, syn::Token![,]>),
    Name(syn::Token![=], Box<syn::Expr>),
}

impl Parse for Named<FnOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "before" => {
                let inner;
                let paren = syn::parenthesized!(inner in input);
                FnOpt::Before(paren, Punctuated::parse_terminated(&inner)?)
            }
            "after" => {
                let inner;
                let paren = syn::parenthesized!(inner in input);
                FnOpt::After(paren, Punctuated::parse_terminated(&inner)?)
            }
            "name" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let name = input.parse::<syn::Expr>()?;
                FnOpt::Name(eq, Box::new(name))
            }
            _ => return Err(Error::new_spanned(&name, "Unknown attribute")),
        };

        Ok(Named { name, value })
    }
}

enum ArgOpt {
    Param,
    Local(syn::Token![=], Box<syn::Expr>),
    Global,
}

impl Parse for Named<ArgOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "param" => ArgOpt::Param,
            "local" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let default = input.parse::<syn::Expr>()?;
                ArgOpt::Local(eq, Box::new(default))
            }
            "global" => ArgOpt::Global,
            _ => return Err(Error::new_spanned(&name, "Unknown attribute")),
        };
        Ok(Named { name, value })
    }
}
