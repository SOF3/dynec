use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Error, Result,
};

pub(crate) fn imp(args: TokenStream, input: TokenStream, subroutine: bool) -> Result<TokenStream> {
    let input: syn::ItemFn = syn::parse2(input)?;
    let ident = &input.sig.ident;
    let vis = &input.vis;
    let other_attrs = &input.attrs;

    if !args.is_empty() {
        let args = syn::parse2::<Attr>(args)?;

        for opt in &args.options {
            match opt.opt {}
        }
    }

    let mut reads = Vec::new();
    let mut writes = Vec::new();
    let mut read_globals = Vec::new();
    let mut write_globals = Vec::new();
    let mut supers = Vec::new();

    let param = {
        let params = &input.sig.inputs;
        if !subroutine && params.len() != 1 {
            return Err(syn::Error::new_spanned(
                params,
                "system functions must have exactly one parameter",
            ));
        }

        let param = &params[0];

        let pat_ty = match param {
            syn::FnArg::Typed(pat_ty) => pat_ty,
            syn::FnArg::Receiver(..) => {
                return Err(syn::Error::new_spanned(
                    param,
                    "system functions cannot have receivers",
                ));
            }
        };

        let bounds = match &*pat_ty.ty {
            syn::Type::ImplTrait(ty) => &ty.bounds,
            _ => return Err(syn::Error::new_spanned(
                &pat_ty.ty,
                "The parameter of system functions must be in the form `impl Reads<A, C> + Writes<A, C> + Super<A, C>`",
            )),
        };
        for bound in bounds {
            let bound = match bound {
                syn::TypeParamBound::Trait(bound) => bound,
                _ => {
                    return Err(syn::Error::new_spanned(
                        bound,
                        "Lifetimes are not allowed in system function signature",
                    ))
                }
            };

            if !matches!(bound.modifier, syn::TraitBoundModifier::None) {
                return Err(syn::Error::new_spanned(
                    &bound.modifier,
                    "The `?` modifier is not allowed in system function signature",
                ));
            }

            if bound.lifetimes.is_some() {
                return Err(syn::Error::new_spanned(
                    &bound.modifier,
                    "Higher-rank trait bounds are not allowed in system function signature",
                ));
            }

            let last = bound
                .path
                .segments
                .last()
                .expect("Path segments cannot be empty");
            let which = last.ident.to_string();

            fn extract_type<'t>(
                args: &'t syn::PathArguments,
                index: usize,
                err: &str,
            ) -> Result<&'t syn::Type> {
                let args = match args {
                    syn::PathArguments::AngleBracketed(args) => &args.args,
                    _ => return Err(syn::Error::new_spanned(&args, err)),
                };

                let ty = match args.iter().nth(index) {
                    Some(syn::GenericArgument::Type(ty)) => ty,
                    _ => return Err(syn::Error::new_spanned(&args, err)),
                };

                Ok(ty)
            }

            match which.as_str() {
                "Context" => {},
                "Reads" | "Writes" => {
                    let arch = extract_type(&last.arguments, 0, "`Reads` and `Writes` should have the forms `Reads<A, C>` and `Writes<A, C>`")?;
                    let comp = extract_type(&last.arguments, 1, "`Reads` and `Writes` should have the forms `Reads<A, C>` and `Writes<A, C>`")?;

                    reads.push((arch, comp));
                    if which == "Writes" {
                        writes.push((arch, comp));
                    }
                },
                "ReadsGlobal" | "WritesGlobal" => {
                    let ty = extract_type(&last.arguments, 0, "`ReadsGlobal` and `WritesGlobal` should have the forms `ReadsGlobal<R>` and `WritesGlobal<R>`")?;

                    read_globals.push(ty);
                    if which == "WritesGlobal" {
                        write_globals.push(ty);
                    }
                },
                "Super" => {
                    let ty = extract_type(&last.arguments, 0, "`Super` should have the form `Super<subsystem>`")?;
                    supers.push(ty);
                },
                _ => return Err(syn::Error::new_spanned(
                    &last.ident,
                    "Only `Context`, `Reads`, `Writes`, `ReadsGlobal`, `WritesGlobal` and `Super` are allowed in the trait bounds",
                )),
            }
        }

        &*pat_ty.pat
    };

    if !subroutine && !matches!(input.sig.output, syn::ReturnType::Default) {
        return Err(syn::Error::new_spanned(
            &input.sig.output,
            "system functions must have a void return type",
        ));
    }

    let fn_body = &*input.block;

    let read_impls = reads.iter().map(|&(arch, comp)| {
        quote! {
            impl ::dynec::system::Reads<#arch, #comp> for __dynec_Context {}
        }
    });
    let write_impls = writes.iter().map(|&(arch, comp)| {
        quote! {
            impl ::dynec::system::Writes<#arch, #comp> for __dynec_Context {}
        }
    });

    let meta = quote! {
        fn meta() -> ::dynec::system::Meta {
            todo!()
        }
    };

    let runner = if subroutine {
        let other_args = input.sig.inputs.iter().skip(1);

        let other_args_types: Vec<_> = other_args
            .clone()
            .map(|arg| match arg {
                syn::FnArg::Typed(pat_ty) => Ok(&pat_ty.ty),
                syn::FnArg::Receiver(..) => Err(syn::Error::new_spanned(
                    arg,
                    "system functions cannot have receivers",
                )),
            })
            .collect::<Result<_>>()?;

        let ret_ty = &input.sig.output;

        let deref_sig = quote!(fn(__dynec_Context, #(#other_args_types),*) #ret_ty);

        quote! {
            impl ::dynec::system::Subroutine for #ident {
                type Context = __dynec_Context;

                #meta
            }

            fn __dynec_run(#param: __dynec_Context, #(#other_args),*) #ret_ty #fn_body

            impl ::std::ops::Deref for #ident {
                type Target = #deref_sig;

                fn deref(&self) -> &Self::Target {
                    const __DYNEC_RUN_CONST: #deref_sig = __dynec_run;
                    &__DYNEC_RUN_CONST
                }
            }
        }
    } else {
        quote! {
            impl ::dynec::system::System for #ident {
                type Context = __dynec_Context;

                fn run(#param: &__dynec_Context) #fn_body

                #meta
            }
        }
    };

    Ok(quote! {
        #(#[#other_attrs])*
        #[allow(non_camel_case_types)]
        #vis struct #ident;

        const _: () = {
            #runner

            struct __dynec_Context {
            }

            impl ::dynec::system::Context for __dynec_Context {
            }

            #(#read_impls)*
            #(#write_impls)*

            #( impl ::dynec::system::ReadsGlobal<#read_globals> for __dynec_Context {})*
            #( impl ::dynec::system::WritesGlobal<#write_globals> for __dynec_Context {})*
            #( impl ::dynec::system::Super<#supers> for __dynec_Context {})*
        };
    })
}

struct Attr {
    options: Punctuated<NamedOpt, syn::Token![,]>,
}

impl Parse for Attr {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Attr {
            options: Punctuated::parse_separated_nonempty(input)?,
        })
    }
}

struct NamedOpt {
    name: syn::Ident,
    opt: Opt,
}

enum Opt {}

impl Parse for NamedOpt {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        #[allow(clippy::match_single_binding)]
        let opt = match name_string.as_str() {
            _ => return Err(Error::new_spanned(&name, "Unknown attribute")),
        };

        Ok(NamedOpt { name, opt })
    }
}
