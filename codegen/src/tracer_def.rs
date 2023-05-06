use std::iter;

use itertools::Itertools;
use matches2::option_match;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::Error;

use crate::util::{self, Attr, Named, Result};

pub(crate) fn imp(args: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let args: Attr<TraitOpt> = syn::parse2(args)?;
    let input: syn::ItemTrait = syn::parse2(input)?;
    let mut mut_input = input.clone();

    let debug_print =
        args.find_one(|opt| option_match!(opt, TraitOpt::DebugPrint => &()))?.is_some();

    let (_, max_tuple_len) = args
        .find_one(|opt| option_match!(opt, TraitOpt::MaxTupleLength(_, int) => int))?
        .ok_or_else(|| syn::Error::new(Span::call_site(), "missing max_tuple_len argument"))?;
    let max_tuple_len = max_tuple_len.base10_parse()?;

    let imports = args
        .items
        .iter()
        .filter_map(|opt| option_match!(&opt.value, TraitOpt::Import(_, import) => import))
        .map(|path| {
            let segments: Vec<_> = path.segments.iter().collect();
            if segments[0].ident == "crate" {
                let mut out = quote!($crate);
                for &segment in &segments[1..] {
                    out.extend(quote!(::#segment));
                }
                out
            } else {
                quote!(#path)
            }
        });

    let tracer_ident = &input.ident;

    let mut noop_items = Vec::new();
    let mut log_items = Vec::new();

    let mut tuple_item_lists: Vec<Vec<TokenStream>> =
        (0..=max_tuple_len).map(|_| Vec::new()).collect();

    let mut polyfill_macro_data = TokenStream::new();

    for item in &mut mut_input.items {
        match item {
            syn::TraitItem::Type(item) => {
                let item_ident = &item.ident;

                let attrs = util::parse_attrs::<AssocTypeOpt>(&mut item.attrs)?;

                let log_ty = if let Some((_, ())) =
                    attrs.find_one(|opt| option_match!(opt, AssocTypeOpt::LogTime => &()))?
                {
                    quote!(std::time::Instant)
                } else {
                    quote!(())
                };

                noop_items.push(quote! {
                    type #item_ident = ();
                });
                log_items.push(quote! {
                    type #item_ident = #log_ty;
                });
                for (i, tuple_items) in tuple_item_lists.iter_mut().enumerate() {
                    let tuple_generics = tuple_generics(i);

                    tuple_items.push(quote! {
                        type #item_ident = (#(#tuple_generics::#item_ident,)*);
                    });
                }

                polyfill_macro_data.extend(quote! {
                    type #item_ident
                });
            }
            syn::TraitItem::Fn(item) => {
                let item_ident = &item.sig.ident;

                let attrs = util::parse_attrs::<AssocFnOpt>(&mut item.attrs)?;

                let log_return_value = if let Some((_, ())) =
                    attrs.find_one(|opt| option_match!(opt, AssocFnOpt::LogReturnNow => &()))?
                {
                    quote!(std::time::Instant::now())
                } else {
                    quote!(())
                };

                let mut param_idents = Vec::new();
                let mut param_tys = Vec::new();
                let mut log_fmts = Vec::new();
                let mut log_exprs = Vec::new();

                for param in &mut item.sig.inputs {
                    let param = match param {
                        syn::FnArg::Typed(typed) => typed,
                        syn::FnArg::Receiver(_) => continue,
                    };

                    let param_attrs = util::parse_attrs::<AssocFnParamOpt>(&mut param.attrs)?;

                    let param_ident = match &*param.pat {
                        syn::Pat::Ident(ident) => &ident.ident,
                        _ => {
                            return Err(syn::Error::new_spanned(
                                &param.pat,
                                "Only identifiers are allowed in receivers",
                            ))
                        }
                    };
                    param_idents.push(param_ident);
                    param_tys.push(&param.ty);

                    if let Some((_, ())) = param_attrs
                        .find_one(|opt| option_match!(opt, AssocFnParamOpt::LogSkip => &()))?
                    {
                        continue;
                    }

                    let mut log_fmt = "{:?}";
                    let mut log_expr = quote!(#param_ident);
                    if let Some((_, wrapper)) = param_attrs.find_one(
                        |opt| option_match!(opt, AssocFnParamOpt::LogWith(_, wrapper) => wrapper),
                    )? {
                        log_fmt = "{}";
                        log_expr = quote!((#wrapper)(#param_ident));
                    }

                    log_fmts.push(log_fmt);
                    log_exprs.push(log_expr);
                }

                let log_fmt_joined = iter::zip(&param_idents, &log_fmts)
                    .map(|(ident, fmt)| format!("{ident} = {fmt}"))
                    .join(", ");
                let log_fmt_str = format!("{item_ident}({log_fmt_joined})");

                let return_ty = &item.sig.output;

                let noop_item = quote! {
                    fn #item_ident(&self, #(
                        _: #param_tys,
                    )*) {}
                };
                noop_items.push(noop_item.clone());
                log_items.push(quote! {
                    #[allow(clippy::unused_unit)]
                    fn #item_ident(&self, #(
                        #[allow(unused_variables)] #param_idents: #param_tys,
                    )*) #return_ty {
                        log::log!(self.0, #log_fmt_str, #(#log_exprs,)*);
                        #log_return_value
                    }
                });
                for (i, tuple_items) in tuple_item_lists.iter_mut().enumerate() {
                    let tuple = (0..i).map(proc_macro2::Literal::usize_unsuffixed).map(|field| {
                        let param_list =
                            iter::zip(&param_idents, &param_tys).map(|(&ident, &ty)| match &**ty {
                                syn::Type::Path(path) => match path.path.segments.first() {
                                    // References an associated type, needs destructuring
                                    Some(first) if first.ident == "Self" => quote!(#ident.#field),
                                    _ => quote!(#ident),
                                },
                                _ => quote!(#ident),
                            });

                        quote!(self.0.#field.#item_ident(#(#param_list,)*))
                    });

                    let ret_suffix = match return_ty {
                        syn::ReturnType::Default => quote!(),
                        syn::ReturnType::Type(_, _) => quote!(_ret),
                    };

                    tuple_items.push(quote! {
                        fn #item_ident(&self, #(
                            #[allow(unused_variables)] #param_idents: #param_tys,
                        )*) #return_ty {
                            let _ret = (#(#tuple,)*);
                            #ret_suffix
                        }
                    });
                }

                polyfill_macro_data.extend(quote! {
                    fn #item_ident { #noop_item }
                });
            }
            _ => return Err(syn::Error::new_spanned(item, "unsupported trait item")),
        }
    }

    let tuple_impls = tuple_item_lists.into_iter().enumerate().map(|(i, items)| {
        let tuple_generics = tuple_generics(i);

        quote! {
            impl<#(#tuple_generics,)*> #tracer_ident for Aggregate<(#(#tuple_generics,)*)>
            where
                #(#tuple_generics: #tracer_ident,)*
            {
                #(#items)*
            }
        }
    });

    let output = quote! {
        #mut_input

        impl #tracer_ident for Log { #(#log_items)* }

        impl #tracer_ident for Noop { #(#noop_items)* }

        #(#tuple_impls)*

        /// This macro is internal and should only be called from the `#[tracer]` macro.
        ///
        /// `#[tracer] impl` => `polyfill_tracer_decl!(impl)`  => `polyfill_tracer_proc!(impl, data)`
        #[doc(hidden)]
        #[macro_export]
        macro_rules! polyfill_tracer_decl {
            ($debug_print:literal $impl_item:block) => {
                const _: () = {
                    #(use #imports;)*
                    use $crate::tracer::*;

                    $crate::polyfill_tracer_proc! {
                        { $crate }
                        $debug_print
                        $impl_item
                        #polyfill_macro_data
                    }
                };
            }
        }
    };
    if debug_print {
        println!("output: {output}");
    }
    Ok(output)
}

fn tuple_generics(i: usize) -> Vec<proc_macro2::Ident> {
    (0..i).map(|j| quote::format_ident!("Item{j}")).collect()
}

enum TraitOpt {
    DebugPrint,
    MaxTupleLength(syn::Token![=], syn::LitInt),
    Import(syn::Token![=], syn::Path),
}

impl Parse for Named<TraitOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;

        let value = match name.to_string().as_str() {
            "__debug_print" => TraitOpt::DebugPrint,
            "max_tuple_len" => {
                let eq = input.parse()?;
                let int = input.parse()?;
                TraitOpt::MaxTupleLength(eq, int)
            }
            "import" => {
                let eq = input.parse()?;
                let path = input.parse()?;
                TraitOpt::Import(eq, path)
            }
            _ => return Err(Error::new_spanned(&name, format!("Unknown argument `{}`", name))),
        };

        Ok(Named { name, value })
    }
}

enum AssocTypeOpt {
    LogTime,
}

impl Parse for Named<AssocTypeOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;

        let value = match name.to_string().as_str() {
            "log_time" => AssocTypeOpt::LogTime,
            _ => return Err(Error::new_spanned(&name, format!("Unknown argument `{}`", name))),
        };

        Ok(Named { name, value })
    }
}

enum AssocFnOpt {
    LogReturnNow,
}

impl Parse for Named<AssocFnOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;

        let value = match name.to_string().as_str() {
            "log_return_now" => AssocFnOpt::LogReturnNow,
            _ => return Err(Error::new_spanned(&name, format!("Unknown argument `{}`", name))),
        };

        Ok(Named { name, value })
    }
}

#[allow(clippy::enum_variant_names)] // TODO remove this when we add more implementations
enum AssocFnParamOpt {
    LogSkip,
    LogElapsedDuration,
    LogWith(syn::Token![=], syn::Expr),
}

impl Parse for Named<AssocFnParamOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;

        let value = match name.to_string().as_str() {
            "log_skip" => AssocFnParamOpt::LogSkip,
            "log_elapsed_duration" => AssocFnParamOpt::LogElapsedDuration,
            "log_with" => {
                let eq = input.parse()?;
                let wrapper = input.parse()?;
                AssocFnParamOpt::LogWith(eq, wrapper)
            }
            _ => return Err(Error::new_spanned(&name, format!("Unknown argument `{}`", name))),
        };

        Ok(Named { name, value })
    }
}
