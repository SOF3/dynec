use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Error, Result};

pub(crate) fn imp(args: TokenStream, input: TokenStream, subroutine: bool) -> Result<TokenStream> {
    let input: syn::ItemFn = syn::parse2(input)?;
    let ident = &input.sig.ident;
    let vis = &input.vis;
    let other_attrs = &input.attrs;

    let mut deps = Vec::new();

    if !args.is_empty() {
        let args = syn::parse2::<Attr>(args)?;

        for opt in &args.options {
            match &opt.opt {
                Opt::Depend(_, opt_deps) => {
                    deps.extend(opt_deps.clone());
                }
            }
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
            _ => {
                return Err(syn::Error::new_spanned(
                    &pat_ty.ty,
                    "The parameter of system functions must be in the form `impl Reads<A, C> + \
                     Writes<A, C> + Super<A, C>`",
                ))
            }
        };
        for bound in bounds {
            let path = extract_bound(bound)?;
            let which = path.ident.to_string();

            match which.as_str() {
                "Context" => {}
                "Reads" | "Writes" => {
                    let arch = extract_type(
                        &path.arguments,
                        0,
                        "`Reads` and `Writes` should have the forms `Reads<A, C>` and `Writes<A, \
                         C>`",
                    )?;
                    let comp = extract_type(
                        &path.arguments,
                        1,
                        "`Reads` and `Writes` should have the forms `Reads<A, C>` and `Writes<A, \
                         C>`",
                    )?;

                    match which.as_str() {
                        "Reads" => reads.push((arch, comp)),
                        "Writes" => writes.push((arch, comp)),
                        _ => unreachable!(),
                    }
                }
                "ReadsGlobal" | "WritesGlobal" => {
                    let ty = extract_type(
                        &path.arguments,
                        0,
                        "`ReadsGlobal` and `WritesGlobal` should have the forms `ReadsGlobal<R>` \
                         and `WritesGlobal<R>`",
                    )?;

                    match which.as_str() {
                        "ReadsGlobal" => read_globals.push(ty),
                        "WritesGlobal" => write_globals.push(ty),
                        _ => unreachable!(),
                    }
                }
                "Super" => {
                    let ty = extract_type(
                        &path.arguments,
                        0,
                        "`Super` should have the form `Super<subsystem>`",
                    )?;
                    supers.push(ty);
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        &path.ident,
                        "Only `Context`, `Reads`, `Writes`, `ReadsGlobal`, `WritesGlobal` and \
                         `Super` are allowed in the trait bounds",
                    ))
                }
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

    let comp_access_impls = {
        let read_impls = reads.iter().chain(writes.iter()).map(|&(arch, comp)| {
            quote! {
                impl<'t> ::dynec::system::Reads<#arch, #comp> for __dynec_Context<'t> {}
            }
        });
        let write_impls = writes.iter().map(|&(arch, comp)| {
            quote! {
                impl<'t> ::dynec::system::Writes<#arch, #comp> for __dynec_Context<'t> {}
            }
        });

        quote! {
            #(#read_impls)*
            #(#write_impls)*
        }
    };

    let meta = quote! {
        fn meta() -> ::dynec::system::Meta {
            ::dynec::system::Meta {
                name: concat!(module_path!(), "::", stringify!(#ident)),

            }
        }
    };

    let runner = {
        let (impl_main, other_args, ret_ty, deref_sig);

        if subroutine {
            other_args = input.sig.inputs.iter().skip(1).collect::<Vec<_>>();

            let other_args_types: Vec<_> = other_args
                .iter()
                .map(|&arg| match arg {
                    syn::FnArg::Typed(pat_ty) => Ok(&pat_ty.ty),
                    syn::FnArg::Receiver(..) => {
                        Err(syn::Error::new_spanned(arg, "system functions cannot have receivers"))
                    }
                })
                .collect::<Result<_>>()?;

            ret_ty = match &input.sig.output {
                syn::ReturnType::Default => quote!(),
                syn::ReturnType::Type(arrow, ty) => quote!(#arrow #ty),
            };

            deref_sig = quote!(for<'t> fn(__dynec_Context<'t>, #(#other_args_types),*) #ret_ty);

            impl_main = quote! {
                impl<'t> ::dynec::system::Subroutine<'t> for $ident {
                    type Context = __dynec_Context<'t>;

                    #meta
                }
            };
        } else {
            other_args = Vec::new();
            ret_ty = quote!();
            deref_sig = quote!(for<'t> fn(__dynec_Context<'t>));

            impl_main = quote! {
                impl ::dynec::System for #ident {
                    fn run(world: &::dynec::World) {
                        let ctx = __dynec_Context::from_world(world);
                        __dynec_run(ctx)
                    }

                    #meta
                }
            };
        }

        quote! {
            #impl_main

            #[inline(always)]
            fn __dynec_run<'t>(#param: __dynec_Context<'t>, #(#other_args),*) #ret_ty #fn_body

            impl ::std::ops::Deref for #ident {
                type Target = #deref_sig;

                fn deref(&self) -> &Self::Target {
                    const __DYNEC_RUN_CONST: #deref_sig = __dynec_run;
                    &__DYNEC_RUN_CONST
                }
            }
        }
    };

    let context_def = {
        let read_storages =
            reads.iter().map(|&(_arch, comp)| quote!(&'t ::std::cell::RefCell<&'t #comp>));
        let write_storages =
            writes.iter().map(|&(_arch, comp)| quote!(&'t ::std::cell::RefCell<&'t mut #comp>));

        quote! {
            struct __dynec_Context<'t>(
                #(#read_storages,)*
                #(#write_storages,)*
            );
        }
    };

    Ok(quote! {
        #(#[#other_attrs])*
        #[allow(non_camel_case_types)]
        #vis struct #ident;

        const _: () = {
            #runner

            #context_def

            impl ::dynec::system::Context for __dynec_Context {}

            #comp_access_impls

            #( impl ::dynec::system::ReadsGlobal<#read_globals> for __dynec_Context {})*
            #( impl ::dynec::system::WritesGlobal<#write_globals> for __dynec_Context {})*
            #( impl ::dynec::system::Super<#supers> for __dynec_Context {})*
        };
    })
}

fn extract_bound(bound: &syn::TypeParamBound) -> Result<&syn::PathSegment> {
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
        _ => return Err(syn::Error::new_spanned(&args, err)),
    };

    let ty = match args.iter().nth(index) {
        Some(syn::GenericArgument::Type(ty)) => ty,
        _ => return Err(syn::Error::new_spanned(&args, err)),
    };

    Ok(ty)
}

struct Attr {
    options: Punctuated<NamedOpt, syn::Token![,]>,
}

impl Parse for Attr {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Attr { options: Punctuated::parse_separated_nonempty(input)? })
    }
}

struct NamedOpt {
    #[allow(dead_code)]
    name: syn::Ident,
    opt:  Opt,
}

enum Opt {
    Depend(syn::token::Paren, Punctuated<syn::Path, syn::Token![,]>),
}

impl Parse for NamedOpt {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        #[allow(clippy::match_single_binding)]
        let opt = match name_string.as_str() {
            "depend" => {
                let inner;
                let paren = syn::parenthesized!(inner in input);
                Opt::Depend(paren, Punctuated::parse_terminated(&inner)?)
            }
            _ => return Err(Error::new_spanned(&name, "Unknown attribute")),
        };

        Ok(NamedOpt { name, opt })
    }
}
