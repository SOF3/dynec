use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::Error;

use crate::entity_ref;
use crate::util::Result;

mod arg;
use arg::ArgType;
mod item;
mod opt;

pub(crate) fn imp(args: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let mut input: syn::ItemFn = syn::parse2(input)?;
    let ident = &input.sig.ident;
    let vis = &input.vis;
    let other_attrs = &input.attrs;

    if !matches!(input.sig.output, syn::ReturnType::Default) {
        return Err(Error::new_spanned(&input.sig.output, "system functions must return unit"));
    }

    // 1. Parse item-level attributes.
    let item = item::Agg::parse(ident, args)?;
    let item::Agg { crate_name, name, state_maybe_uninit, deps, .. } = item;

    // 2. Parse parameters.

    let mut local_state_field_idents: Vec<syn::Ident> = Vec::new();
    let mut local_state_field_pats: Vec<Box<syn::Pat>> = Vec::new();
    let mut local_state_field_tys: Vec<syn::Type> = Vec::new();
    let mut local_state_entity_attrs: Vec<TokenStream> = Vec::new();

    let mut param_state_field_idents: Vec<syn::Ident> = Vec::new();
    let mut param_state_field_tys: Vec<syn::Type> = Vec::new();

    let mut initial_state_field_idents: Vec<syn::Ident> = Vec::new();
    let mut initial_state_field_defaults: Vec<Box<syn::Expr>> = Vec::new();

    let mut isotope_discrim_idents: Vec<syn::Ident> = Vec::new();
    let mut isotope_discrim_ty_params: Vec<syn::Ident> = Vec::new();
    let mut isotope_discrim_type_bounds: Vec<TokenStream> = Vec::new();
    let mut isotope_discrim_values: Vec<Box<syn::Expr>> = Vec::new();

    let mut input_types: Vec<syn::Type> = Vec::new();
    let mut system_run_args: Vec<TokenStream> = Vec::new();

    let mut global_requests: Vec<TokenStream> = Vec::new();
    let mut simple_requests: Vec<TokenStream> = Vec::new();
    let mut isotope_requests: Vec<TokenStream> = Vec::new();
    let mut entity_creator_requests: Vec<TokenStream> = Vec::new();

    for (param_index, param) in input.sig.inputs.iter_mut().enumerate() {
        let param = match param {
            syn::FnArg::Receiver(receiver) => {
                return Err(Error::new_spanned(receiver, "system functions must not be a method"))
            }
            syn::FnArg::Typed(typed) => typed,
        };

        input_types.push(syn::Type::clone(&param.ty));

        let arg_type = arg::infer_arg_type(param)?;

        let run_arg = match arg_type {
            ArgType::Local { default, referrer_attr } => {
                let referrer_attr = match referrer_attr {
                    Some(true) => quote!(#[entity]),
                    Some(false) => quote!(#[not_entity]),
                    None => quote!(),
                };

                let field_name = match &*param.pat {
                    syn::Pat::Ident(ident) => ident.ident.clone(),
                    syn::Pat::Reference(pat) => match &*pat.pat {
                        syn::Pat::Ident(ident) => ident.ident.clone(),
                        _ => quote::format_ident!("__dynec_local_{}", param_index),
                    },
                    _ => quote::format_ident!("__dynec_local_{}", param_index),
                };

                let param_ty = match &*param.ty {
                    syn::Type::Reference(ty) => &ty.elem,
                    _ => {
                        return Err(Error::new_spanned(
                            &param.ty,
                            "#[local] and #[param] can only be used on reference type parameters",
                        ))
                    }
                };

                local_state_field_idents.push(field_name.clone());
                local_state_field_pats.push(param.pat.clone());
                local_state_field_tys.push(syn::Type::clone(param_ty));
                local_state_entity_attrs.push(referrer_attr);

                if let Some(default) = default {
                    initial_state_field_idents.push(field_name.clone());
                    initial_state_field_defaults.push(default);
                } else {
                    param_state_field_idents.push(field_name.clone());
                    param_state_field_tys.push(syn::Type::clone(param_ty));
                }

                quote! {
                    #[allow(clippy::unnecessary_mut_passed)]
                    {
                        &mut self.#field_name
                    }
                }
            }
            ArgType::Global { thread_local, maybe_uninit } => {
                if thread_local && !item.system_thread_local {
                    return Err(Error::new_spanned(
                        param,
                        "Thread-local global states can only be used in systems marked as \
                         `#[dynec(thread_local)]`.",
                    ));
                }

                let new_sync = match thread_local {
                    true => quote!(new_unsync),
                    false => quote!(new_sync),
                };

                let (param_ty, mutable) = match &*param.ty {
                    syn::Type::Reference(ty) => (&ty.elem, ty.mutability.is_some()),
                    _ => {
                        return Err(Error::new_spanned(
                            &param.ty,
                            "#[global] can only be used on reference type parameters",
                        ))
                    }
                };

                global_requests.push(quote! {
                    #crate_name::system::spec::GlobalRequest::#new_sync::<#param_ty>(#mutable)
                        #(.maybe_uninit::<#maybe_uninit>())*
                });

                match (thread_local, mutable) {
                    (false, true) => quote!(&mut *sync_globals.write::<#param_ty>()),
                    (false, false) => quote!(&*sync_globals.read::<#param_ty>()),
                    (true, _) => quote!(unsync_globals.get::<#param_ty>()),
                }
            }
            ArgType::Simple { mutable, arch, comp, maybe_uninit } => {
                simple_requests.push(quote! {
                    #crate_name::system::spec::SimpleRequest::new::<#arch, #comp>(#mutable)
                        #(.maybe_uninit::<#maybe_uninit>())*
                });

                match mutable {
                    true => quote!(components.write_simple_storage::<#arch, #comp>()),
                    false => quote!(components.read_simple_storage::<#arch, #comp>()),
                }
            }
            ArgType::Isotope { mutable, arch, comp, discrim, discrim_key, maybe_uninit } => {
                let discrim_field = if let Some(discrim) = discrim {
                    let discrim_key = match discrim_key {
                        Ok(ty) => ty,
                        Err(span) => {
                            return Err(Error::new(
                                span,
                                "Type parameter `K` must be specified for \
                                 `ReadIsotope`/`WriteIsotope` if partial isotope access is used",
                            ))
                        }
                    };

                    let discrim_ident =
                        quote::format_ident!("__dynec_isotope_discrim_{}", param_index);
                    isotope_discrim_idents.push(discrim_ident.clone());
                    isotope_discrim_ty_params.push(quote::format_ident!(
                        "__DynecDiscrimType{}",
                        isotope_discrim_ty_params.len()
                    ));
                    isotope_discrim_type_bounds.push(quote!(
                        #crate_name::comp::discrim::Set<
                            <#comp as #crate_name::comp::Isotope<#arch>>::Discrim,
                            Key = #discrim_key,
                        >));
                    isotope_discrim_values.push(discrim);

                    Some(quote!(self.__dynec_isotope_discrim_idents.#discrim_ident))
                } else {
                    None
                };

                let discrim_field_variadic = discrim_field.as_ref().map(|expr| quote!(&#expr));
                let discrim_value_option = match &discrim_field {
                    Some(expr) => {
                        quote!(Some({
                            let __iter = #crate_name::comp::discrim::Set::iter_discrims(&#expr);
                            let __iter = ::std::iter::Iterator::map(__iter, |d| {
                                let d: &<#comp as #crate_name::comp::Isotope<#arch>>::Discrim = &d; // auto deref
                                #crate_name::comp::Discrim::into_usize(*d)
                            });
                            ::std::iter::Iterator::collect::<::std::vec::Vec<_>>(__iter)
                        }))
                    }
                    None => quote!(None),
                };

                isotope_requests.push(quote! {
                    #crate_name::system::spec::IsotopeRequest::new::<#arch, #comp>(#discrim_value_option, #mutable)
                        #(.maybe_uninit::<#maybe_uninit>())*
                });

                let method_ident = match (mutable, discrim_field.is_some()) {
                    (true, true) => quote!(write_partial_isotope_storage::<#arch, #comp, _>),
                    (true, false) => quote!(write_full_isotope_storage::<#arch, #comp>),
                    (false, true) => quote!(read_partial_isotope_storage::<#arch, #comp, _>),
                    (false, false) => quote!(read_full_isotope_storage::<#arch, #comp>),
                };

                quote!(components.#method_ident(#discrim_field_variadic))
            }
            ArgType::EntityCreator { arch, no_partition } => {
                let no_partition_call = no_partition.then(|| quote!(.no_partition()));
                entity_creator_requests.push(quote! {
                    #crate_name::system::spec::EntityCreatorRequest::new::<#arch>()
                        #no_partition_call
                });

                quote!(#crate_name::system::EntityCreatorImpl {
                    buffer: &offline_buffer,
                    ealloc: ealloc_shard_map.borrow::<#arch>(),
                })
            }
            ArgType::EntityDeleter { arch } => {
                quote!(#crate_name::system::EntityDeleterImpl::<#arch> {
                    buffer: &offline_buffer,
                    _ph: ::std::marker::PhantomData,
                })
            }
        };
        system_run_args.push(run_arg);
    }

    // 3. Generate code.

    let fn_body = &*input.block;

    let input_args: Vec<_> = input.sig.inputs.iter().collect();

    let input_proxy_args: Vec<_> =
        (0..input.sig.inputs.len()).map(|i| quote::format_ident!("arg_{}", i)).collect();

    // Destructure all local states into local variables.
    let destructure_local_states = quote! {
        #[allow(unused_variables, clippy::unused_unit)]
        let (#(#local_state_field_pats,)*) = {
            let Self {
                #(#local_state_field_idents,)*
                __dynec_isotope_discrim_idents: _,
            } = self;
            (#(#local_state_field_idents,)*)
        };
    };

    let isotope_discrim_idents_struct = quote! {
        #[allow(non_camel_case_types)]
        struct __dynec_isotope_discrim_idents<#(
            #isotope_discrim_ty_params: #isotope_discrim_type_bounds,
        )*> {
            #(#isotope_discrim_idents: #isotope_discrim_ty_params,)*
        }
    };
    let mut local_state_struct = syn::parse2(quote! {
        #[allow(non_camel_case_types)]
        struct __dynec_local_state<#(
            #isotope_discrim_ty_params: #isotope_discrim_type_bounds,
        )*> {
            #(#local_state_entity_attrs #local_state_field_idents: #local_state_field_tys,)*
            #[not_entity = "no entities can be assigned in discriminants because the world is not created yet."]
            __dynec_isotope_discrim_idents: __dynec_isotope_discrim_idents<#(#isotope_discrim_ty_params,)*>,
        }
    }).expect("invalid struct expression");
    let impl_referrer_for_local_state = entity_ref::entity_ref(
        &mut local_state_struct,
        crate_name.clone(),
        quote! {
            this_field_references_an_entity_so_it_should_use_dynec_param_entity_or_dynec_local_entity
        },
    )?;

    let (system_trait, system_run_params) = match item.system_thread_local {
        true => (
            quote!(Unsendable),
            quote! {
                sync_globals: &#crate_name::world::SyncGlobals,
                unsync_globals: &mut #crate_name::world::UnsyncGlobals,
                components: &#crate_name::world::Components,
                ealloc_shard_map: &mut #crate_name::entity::ealloc::ShardMap,
                offline_buffer: &mut #crate_name::world::offline::BufferShard,
            },
        ),
        false => (
            quote!(Sendable),
            quote! {
                sync_globals: &#crate_name::world::SyncGlobals,
                components: &#crate_name::world::Components,
                ealloc_shard_map: &mut #crate_name::entity::ealloc::ShardMap,
                offline_buffer: &mut #crate_name::world::offline::BufferShard,
            },
        ),
    };

    let build_fn = quote! {
        /// Builds an instance of the system with required parameters.
        #vis fn build(
            &self,
            #(#param_state_field_idents: #param_state_field_tys,)*
        ) -> impl #crate_name::system::#system_trait {
            // Parameters are first borrowed here to prepare the discrim set.
            // This block cannot move out any `#param_state_field_idents`
            // because they will be moved into the local state struct in the next
            // statement.
            let __dynec_isotope_discrim_idents = {
                __dynec_isotope_discrim_idents {
                    #(#isotope_discrim_idents: ::std::clone::Clone::clone(&#isotope_discrim_values),)*
                }
            };

            __dynec_local_state {
                __dynec_isotope_discrim_idents,
                #(#param_state_field_idents,)*
                #(#initial_state_field_idents: #initial_state_field_defaults,)*
            }
        }
    };
    let call_fn = quote! {
        /// Calls the underlying system function directly.
        ///
        /// This function should only be used in unit tests.
        #vis fn call(#(#input_proxy_args: #input_types),*) {
            __dynec_original(#(#input_proxy_args),*)
        }
    };

    let impl_descriptor = quote! {
        #[automatically_derived]
        impl<#(
            #isotope_discrim_ty_params: #isotope_discrim_type_bounds,
        )*> #crate_name::system::Descriptor for __dynec_local_state<#(#isotope_discrim_ty_params,)*> {
            fn get_spec(&self) -> #crate_name::system::Spec {
                #destructure_local_states

                #crate_name::system::Spec {
                    debug_name: {
                        ::std::string::String::from(#name)
                    },
                    dependencies: vec![#(#deps),*],
                    global_requests: vec![#(#global_requests),*],
                    simple_requests: vec![#(#simple_requests),*],
                    isotope_requests: vec![#(#isotope_requests),*],
                    entity_creator_requests: vec![#(#entity_creator_requests),*],
                }
            }

            fn visit_type(&self, arg: &mut #crate_name::entity::referrer::VisitTypeArg) {
                <Self as #crate_name::entity::Referrer>::visit_type(arg)
            }

            fn state_maybe_uninit(&self) -> ::std::vec::Vec<::std::any::TypeId> {
                vec![#(#state_maybe_uninit),*]
            }

            fn visit_mut(&mut self) -> #crate_name::entity::referrer::AsObject<'_> {
                #crate_name::entity::referrer::AsObject::of(self)
            }
        }
    };
    let impl_system = quote! {
        #[automatically_derived]
        impl<#(
            #isotope_discrim_ty_params: #isotope_discrim_type_bounds,
        )*> #crate_name::system::#system_trait for __dynec_local_state<#(#isotope_discrim_ty_params,)*> {
            fn run(&mut self, #system_run_params) {
                let offline_buffer = ::std::cell::RefCell::new(offline_buffer);

                __dynec_original(#(#system_run_args),*)
            }

            fn as_descriptor_mut(&mut self) -> &mut dyn #crate_name::system::Descriptor { self }
        }
    };

    let output = quote! {
        // A unit struct that serves as the API entry point.
        #(#[#other_attrs])*
        #[derive(Clone, Copy)]
        #[allow(non_camel_case_types)]
        #vis struct #ident;

        const _: () = {
            // Public API methods.
            #[automatically_derived]
            impl #ident {
                #build_fn
                #call_fn
            }

            #isotope_discrim_idents_struct

            #local_state_struct
            #impl_referrer_for_local_state

            // The actual function is moved here.
            fn __dynec_original(#(#input_args),*) {
                #fn_body
            }

            #impl_descriptor

            #impl_system
        };
    };

    if item.debug_print {
        println!("{}", &output);
    }

    Ok(output)
}

fn parse_maybe_uninit<T>(
    input: ParseStream,
    constructor: fn(syn::token::Paren, Punctuated<syn::Type, syn::Token![,]>) -> T,
) -> Result<T> {
    let inner;
    let paren = syn::parenthesized!(inner in input);
    let punctuated = Punctuated::parse_terminated(&inner)?;
    Ok(constructor(paren, punctuated))
}
