use matches2::option_match;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::Error;

use crate::util::{Attr, Named, Result};
use crate::{entity_ref, util};

pub(crate) fn imp(args: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let args: Attr<ItemOpt> = syn::parse2(args)?;

    let crate_name = if let Some((_, ts)) =
        args.find_one(|opt| option_match!(opt, ItemOpt::DynecAs(_, ts) => ts))?
    {
        ts.clone()
    } else {
        quote!(::dynec)
    };

    let archetypes: Vec<_> = args
        .items
        .iter()
        .filter_map(|arg| match &arg.value {
            ItemOpt::Of(_, ty) => Some(ty),
            _ => None,
        })
        .collect();

    let isotope =
        args.find_one(|arg| option_match!(arg, ItemOpt::Isotope(_, discrim) => discrim))?;
    let storage =
        match args.find_one(|arg| option_match!(arg, ItemOpt::Storage(_, discrim) => discrim))? {
            Some((_, ty)) => ty.clone(),
            None => syn::parse2(quote!(#crate_name::storage::Vec))
                .expect("Cannot parse storage::Vec as a path"),
        };

    let presence = args.find_one(|arg| option_match!(arg, ItemOpt::Required => &()))?;
    if let (Some((isotope_span, _)), Some((presence_span, _))) = (isotope, presence) {
        return Err(Error::new(
            isotope_span.join(presence_span).unwrap_or(presence_span),
            "isotope components cannot be required because new isotopes may be created dynamically",
        ));
    }
    let presence_enum = match presence {
        Some(_) => quote!(#crate_name::comp::SimplePresence::Required),
        None => quote!(#crate_name::comp::SimplePresence::Optional),
    };

    let finalizer = args.find_one(|arg| option_match!(arg, ItemOpt::Finalizer => &()))?;
    if let (Some((isotope_span, _)), Some((finalizer_span, _))) = (isotope, finalizer) {
        return Err(Error::new(
            isotope_span.join(finalizer_span).unwrap_or(finalizer_span),
            "isotope components cannot be finalizers",
        ));
    }
    let finalizer = finalizer.is_some();

    let init = args.find_one(|arg| option_match!(arg, ItemOpt::Init(_, func) => func))?;

    let input: syn::DeriveInput = syn::parse2(input)?;
    let generics = util::parse_generics(&input);

    let mut output = TokenStream::new();
    for archetype in archetypes {
        let storage = if storage.segments.iter().all(|segment| segment.arguments.is_empty()) {
            quote!(#storage<<#archetype as #crate_name::Archetype>::RawEntity, Self>)
        } else {
            quote!(#storage)
        };

        if let Some((_, discrim)) = isotope {
            let init = match init {
                Some((_, func)) => {
                    output.extend(
                        generics.impl_trait(quote!(#crate_name::comp::Must<#archetype>), quote! {}),
                    );

                    let func = func.as_fn_ptr(&generics)?;
                    quote!(#crate_name::comp::IsotopeInitStrategy::Default(#func))
                }
                None => quote!(#crate_name::comp::IsotopeInitStrategy::None),
            };

            output.extend(generics.impl_trait(
                quote!(#crate_name::comp::Isotope<#archetype>),
                quote! {
                    type Discrim = #discrim;

                    const INIT_STRATEGY: #crate_name::comp::IsotopeInitStrategy<Self> = #init;

                    type Storage = #storage;
                },
            ));
        } else {
            let init_strategy = match init {
                Some((_, func)) => {
                    let func = func.as_fn_ptr(&generics)?;
                    quote!(#crate_name::comp::SimpleInitStrategy::Auto(
                        #crate_name::comp::AutoIniter { f: &#func }
                    ))
                }
                None => quote!(#crate_name::comp::SimpleInitStrategy::None),
            };

            output.extend(generics.impl_trait(
                quote!(#crate_name::comp::Simple<#archetype>),
                quote! {
                    const PRESENCE: #crate_name::comp::SimplePresence = #presence_enum;
                    const INIT_STRATEGY: #crate_name::comp::SimpleInitStrategy<#archetype> = #init_strategy;
                    const IS_FINALIZER: bool = #finalizer;

                    type Storage = #storage;
                },
            ));

            if presence.is_some() {
                output.extend(
                    generics.impl_trait(quote!(#crate_name::comp::Must<#archetype>), quote! {}),
                );
            }
        }
    }

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
        #output
        #entity_ref
    })
}

enum ItemOpt {
    DynecAs(syn::token::Paren, TokenStream),
    Of(syn::Token![=], syn::Type),
    Isotope(syn::Token![=], syn::Type),
    Storage(syn::Token![=], syn::Path),
    Required,
    Finalizer,
    Init(syn::Token![=], Box<FunctionRefWithArity>),
}

impl Parse for Named<ItemOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;

        let value = match name.to_string().as_str() {
            "dynec_as" => {
                let inner;
                let paren = syn::parenthesized!(inner in input);
                let args = inner.parse()?;
                ItemOpt::DynecAs(paren, args)
            }
            "of" => {
                let eq: syn::Token![=] = input.parse()?;
                let ty = input.parse::<syn::Type>()?;
                ItemOpt::Of(eq, ty)
            }
            "isotope" => {
                let eq: syn::Token![=] = input.parse()?;
                let ty = input.parse::<syn::Type>()?;
                ItemOpt::Isotope(eq, ty)
            }
            "storage" => {
                let eq: syn::Token![=] = input.parse()?;
                let ty = input.parse::<syn::Path>()?;
                ItemOpt::Storage(eq, ty)
            }
            "required" => ItemOpt::Required,
            "finalizer" => ItemOpt::Finalizer,
            "init" => {
                let eq: syn::Token![=] = input.parse()?;
                let expr = input.parse::<FunctionRefWithArity>()?;
                ItemOpt::Init(eq, Box::new(expr))
            }
            _ => return Err(Error::new_spanned(&name, format!("Unknown argument `{}`", name))),
        };

        Ok(Named { name, value })
    }
}

/// Either a closure or a function reference in the form `path / arity` (e.g. `count/1`).
enum FunctionRefWithArity {
    Closure(syn::ExprClosure),
    Fn(syn::Expr, syn::Token![/], syn::LitInt),
}

impl Parse for FunctionRefWithArity {
    fn parse(input: ParseStream) -> Result<Self> {
        if let Ok(closure) = input.parse::<syn::ExprClosure>() {
            return Ok(FunctionRefWithArity::Closure(closure));
        }

        if let Ok(bin) = input.parse::<syn::ExprBinary>() {
            if let (
                left,
                syn::BinOp::Div(op),
                syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(arity), .. }),
            ) = (&*bin.left, bin.op, &*bin.right)
            {
                return Ok(FunctionRefWithArity::Fn(left.clone(), op, arity.clone()));
            }
        }

        Err(input.error(
            "expected closure or function reference in the form `path/arity` (e.g. \
             `Default::default/0`)",
        ))
    }
}

impl FunctionRefWithArity {
    fn as_fn_ptr(&self, expect_ty: &util::ParsedGenerics) -> Result<TokenStream> {
        let (expr, arity) = match self {
            FunctionRefWithArity::Closure(closure) => (
                {
                    let args = &closure.inputs;
                    let body = &closure.body;

                    let &util::ParsedGenerics { ref ident, ref decl, ref usage, ref where_ } =
                        expect_ty;

                    quote! {{
                        fn __dynec_closure_fn #decl (#args) -> #ident #usage #where_ {
                            #body
                        }

                        __dynec_closure_fn
                    }}
                },
                closure.inputs.len(),
            ),
            FunctionRefWithArity::Fn(expr, _, arity) => {
                (quote!(#expr), arity.base10_parse::<usize>()?)
            }
        };

        let args = (0..arity).map(|_| quote!(&_));

        Ok(quote! {
            (#expr as fn(#(#args),*) -> _)
        })
    }
}
