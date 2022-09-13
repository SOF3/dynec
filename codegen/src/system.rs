use matches2::option_match;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Error, Result};

use crate::entity_ref;
use crate::util::{self, Attr, Named};

pub(crate) fn imp(args: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let mut input: syn::ItemFn = syn::parse2(input)?;
    let ident = &input.sig.ident;
    let vis = &input.vis;
    let other_attrs = &input.attrs;

    let mut deps = Vec::new();

    let mut name = quote!(concat!(module_path!(), "::", stringify!(#ident)));

    let mut crate_name = quote!(::dynec);
    let mut system_thread_local = false;
    let mut state_maybe_uninit = Vec::new();

    if !args.is_empty() {
        let args = syn::parse2::<Attr<FnOpt>>(args)?;

        if let Some((_, ts)) =
            args.find_one(|opt| option_match!(opt, FnOpt::DynecAs(_, ts) => ts))?
        {
            crate_name = ts.clone();
        }

        system_thread_local =
            args.find_one(|opt| option_match!(opt, FnOpt::ThreadLocal => &()))?.is_some();
        state_maybe_uninit = args.merge_all(
            |opt| option_match!(opt, FnOpt::MaybeUninit(_, archs) => archs.iter().cloned()),
        );

        for named in &args.items {
            match &named.value {
                FnOpt::DynecAs(_, _) => {} // already handled
                FnOpt::ThreadLocal => {}   // already handled
                FnOpt::Before(_, inputs) => {
                    for dep in inputs {
                        deps.push(quote!(#crate_name::system::spec::Dependency::Before(Box::new(#dep) as Box<dyn #crate_name::system::Partition>)));
                    }
                }
                FnOpt::After(_, inputs) => {
                    for dep in inputs {
                        deps.push(quote!(#crate_name::system::spec::Dependency::After(Box::new(#dep) as Box<dyn #crate_name::system::Partition>)));
                    }
                }
                FnOpt::Name(_, name_expr) => {
                    name = quote!(#name_expr);
                }
                FnOpt::MaybeUninit(_, _) => {} // already handled
            }
        }
    }

    if !matches!(input.sig.output, syn::ReturnType::Default) {
        return Err(Error::new_spanned(&input.sig.output, "system functions must return unit"));
    }

    let mut local_state_field_idents = Vec::new();
    let mut local_state_field_pats = Vec::new();
    let mut local_state_field_tys = Vec::new();
    let mut local_state_entity_attrs = Vec::new();

    let mut param_state_field_idents = Vec::new();
    let mut param_state_field_tys = Vec::new();

    let mut initial_state_field_idents = Vec::new();
    let mut initial_state_field_defaults = Vec::new();

    let mut isotope_discrim_idents = Vec::new();
    let mut isotope_discrim_values = Vec::new();

    let mut input_types = Vec::new();
    let mut system_run_args = Vec::new();

    let mut global_requests = Vec::new();
    let mut simple_requests = Vec::new();
    let mut isotope_requests = Vec::new();
    let mut entity_creator_requests = Vec::new();

    for (param_index, param) in input.sig.inputs.iter_mut().enumerate() {
        let param = match param {
            syn::FnArg::Receiver(receiver) => {
                return Err(Error::new_spanned(receiver, "system functions must not be a method"))
            }
            syn::FnArg::Typed(typed) => typed,
        };

        input_types.push(syn::Type::clone(&param.ty));

        enum ArgType {
            Local {
                default:    Option<Box<syn::Expr>>,
                has_entity: bool,
            },
            Global {
                thread_local: bool,
                maybe_uninit: Vec<syn::Type>,
            },
            Simple {
                mutable:      bool,
                arch:         Box<syn::Type>,
                comp:         Box<syn::Type>,
                maybe_uninit: Vec<syn::Type>,
            },
            Isotope {
                mutable:      bool,
                arch:         Box<syn::Type>,
                comp:         Box<syn::Type>,
                discrim:      Option<Box<syn::Expr>>,
                maybe_uninit: Vec<syn::Type>,
            },
            EntityCreator {
                arch:         Box<syn::Type>,
                no_partition: bool,
            },
            EntityDeleter {
                arch: Box<syn::Type>,
            },
        }
        type PartialArgTypeBuilder = Box<dyn FnOnce(&str, &[&syn::Type], Span) -> Result<ArgType>>;

        enum MaybeArgType {
            None,
            Some(ArgType),
            Partial { builder: PartialArgTypeBuilder },
        }

        let mut arg_type: MaybeArgType = MaybeArgType::None;

        fn set_arg_type(
            arg_type_ref: &mut MaybeArgType,
            ident: syn::Ident,
            arg_type: ArgType,
        ) -> Result<()> {
            set_maybe_arg_type(arg_type_ref, ident, MaybeArgType::Some(arg_type))
        }
        fn set_partial_arg_type(
            arg_type_ref: &mut MaybeArgType,
            ident: syn::Ident,
            builder: PartialArgTypeBuilder,
        ) -> Result<()> {
            set_maybe_arg_type(arg_type_ref, ident, MaybeArgType::Partial { builder })
        }
        fn set_maybe_arg_type(
            arg_type_ref: &mut MaybeArgType,
            ident: syn::Ident,
            arg_type: MaybeArgType,
        ) -> Result<()> {
            if !(matches!(arg_type_ref, MaybeArgType::None)) {
                return Err(Error::new(
                    ident.span(),
                    "Each argument can only have one argument type",
                ));
            }

            *arg_type_ref = arg_type;
            Ok(())
        }

        fn simple_partial_builder(
            mutable: bool,
            maybe_uninit: Vec<syn::Type>,
        ) -> PartialArgTypeBuilder {
            Box::new(move |ident, args, args_span| {
                let [arch, comp]: [&syn::Type; 2] = args.try_into().map_err(|_| {
                    Error::new(
                        args_span,
                        "Cannot infer archetype and component for component access. Specify \
                         explicitly with `#[dynec(simple(arch = X, comp = Y))]`, or use `impl \
                         ReadSimple<X, Y>`/`impl WriteSimple<X, Y>`.",
                    )
                })?;

                Ok(ArgType::Simple {
                    mutable: mutable || ident == "WriteSimple",
                    arch: Box::new(arch.clone()),
                    comp: Box::new(comp.clone()),
                    maybe_uninit,
                })
            })
        }

        fn isotope_partial_builder(
            mutable: bool,
            discrim: Option<Box<syn::Expr>>,
            maybe_uninit: Vec<syn::Type>,
        ) -> PartialArgTypeBuilder {
            Box::new(move |ident, args, args_span| {
                let [arch, comp]: [&syn::Type; 2] = args.try_into().map_err(|_| {
                    Error::new(
                        args_span,
                        "Cannot infer archetype and component for component access. Specify \
                         explicitly with `#[dynec(isotope(arch = X, comp = Y))]`, or use `impl \
                         ReadIsotope<X, Y>`/`impl WriteIsotope<X, Y>`.",
                    )
                })?;

                Ok(ArgType::Isotope {
                    mutable: mutable || ident == "WriteIsotope",
                    arch: Box::new(arch.clone()),
                    comp: Box::new(comp.clone()),
                    discrim,
                    maybe_uninit,
                })
            })
        }

        fn entity_creator_partial_builder(no_partition: bool) -> PartialArgTypeBuilder {
            Box::new(move |_, args, args_span| {
                let [arch]: [&syn::Type; 1] = args.try_into().map_err(|_| {
                    Error::new(
                        args_span,
                        "Cannot infer archetype for entity creation. Specify explicitly with \
                         `#[dynec(entity_creator(arch = X))]`, or use `impl EntityCreator<X>`.",
                    )
                })?;

                Ok(ArgType::EntityCreator { arch: Box::new(arch.clone()), no_partition })
            })
        }

        fn entity_deleter_partial_builder() -> PartialArgTypeBuilder {
            Box::new(move |_, args, args_span| {
                let [arch]: [&syn::Type; 1] = args.try_into().map_err(|_| {
                    Error::new(
                        args_span,
                        "Cannot infer archetype for entity deletion. Specify explicitly with \
                         `#[dynec(entity_deleter(arch = X))]`, or use `impl EntityDeleter<X>`.",
                    )
                })?;

                Ok(ArgType::EntityDeleter { arch: Box::new(arch.clone()) })
            })
        }

        for attr in util::slow_drain_filter(&mut param.attrs, |attr| attr.path.is_ident("dynec")) {
            let arg_attr = attr.parse_args::<Attr<ArgOpt>>()?;

            for arg in arg_attr.items {
                match arg.value {
                    ArgOpt::Param(_, opts) => {
                        let has_entity = opts
                            .find_one(|opt| option_match!(opt, ParamArgOpt::HasEntity => &()))?
                            .is_some();
                        set_arg_type(
                            &mut arg_type,
                            arg.name,
                            ArgType::Local { default: None, has_entity },
                        )?;
                    }
                    ArgOpt::Local(_, opts) => {
                        let initial = match opts.find_one(
                            |opt| option_match!(opt, LocalArgOpt::Initial(_, initial) => initial),
                        )? {
                            Some((_, initial)) => initial,
                            None => {
                                return Err(Error::new_spanned(
                                    attr,
                                    "Missing required expression for #[dynec(local(initial = \
                                     expr))]",
                                ))
                            }
                        };
                        let has_entity = opts
                            .find_one(|opt| option_match!(opt, LocalArgOpt::HasEntity => &()))?
                            .is_some();
                        set_arg_type(
                            &mut arg_type,
                            arg.name,
                            ArgType::Local { default: Some(initial.clone()), has_entity },
                        )?;
                    }
                    ArgOpt::Global(_, opts) => {
                        let thread_local = opts
                            .find_one(|opt| option_match!(opt, GlobalArgOpt::ThreadLocal => &()))?
                            .is_some();
                        let maybe_uninit = opts.merge_all(|opt| option_match!(opt, GlobalArgOpt::MaybeUninit(_, tys) => tys.iter().cloned()));
                        set_arg_type(
                            &mut arg_type,
                            arg.name,
                            ArgType::Global { thread_local, maybe_uninit },
                        )?;
                    }
                    ArgOpt::Simple(_, opts) => {
                        let mutable = opts
                            .find_one(|opt| option_match!(opt, SimpleArgOpt::Mutable => &()))?
                            .is_some();
                        let arch = opts
                            .find_one(|opt| option_match!(opt, SimpleArgOpt::Arch(_, ty) => ty))?;
                        let comp = opts
                            .find_one(|opt| option_match!(opt, SimpleArgOpt::Comp(_, ty) => ty))?;
                        let maybe_uninit = opts.merge_all(|opt| option_match!(opt, SimpleArgOpt::MaybeUninit(_, tys) => tys.iter().cloned()));

                        match (arch, comp, mutable) {
                            (Some((_, arch)), Some((_, comp)), mutable) => {
                                set_arg_type(
                                    &mut arg_type,
                                    arg.name,
                                    ArgType::Simple {
                                        mutable,
                                        arch: arch.clone(),
                                        comp: comp.clone(),
                                        maybe_uninit,
                                    },
                                )?;
                            }
                            (None, None, false) => {
                                set_partial_arg_type(
                                    &mut arg_type,
                                    arg.name,
                                    simple_partial_builder(mutable, maybe_uninit),
                                )?;
                            }
                            _ => {
                                return Err(Error::new_spanned(
                                    attr,
                                    "Invalid argument. `arch`, `comp` and `mutable` have no \
                                     effect unless both `arch` and `comp` are supplied.",
                                ));
                            }
                        }
                    }
                    ArgOpt::Isotope(_, opts) => {
                        let mutable = opts
                            .find_one(|opt| option_match!(opt, IsotopeArgOpt::Mutable => &()))?
                            .is_some();
                        let arch = opts
                            .find_one(|opt| option_match!(opt, IsotopeArgOpt::Arch(_, ty) => ty))?;
                        let comp = opts
                            .find_one(|opt| option_match!(opt, IsotopeArgOpt::Comp(_, ty) => ty))?;
                        let discrim = opts.find_one(
                            |opt| option_match!(opt, IsotopeArgOpt::Discrim(_, discrim) => discrim),
                        )?;
                        let maybe_uninit = opts.merge_all(|opt| option_match!(opt, IsotopeArgOpt::MaybeUninit(_, tys) => tys.iter().cloned()));

                        match (arch, comp, mutable) {
                            (Some((_, arch)), Some((_, comp)), mutable) => {
                                set_arg_type(
                                    &mut arg_type,
                                    arg.name,
                                    ArgType::Isotope {
                                        mutable,
                                        arch: arch.clone(),
                                        comp: comp.clone(),
                                        discrim: discrim.map(|(_, discrim)| discrim.clone()),
                                        maybe_uninit,
                                    },
                                )?;
                            }
                            (None, None, false) => {
                                set_partial_arg_type(
                                    &mut arg_type,
                                    arg.name,
                                    isotope_partial_builder(
                                        mutable,
                                        discrim.map(|(_, expr)| expr.clone()),
                                        maybe_uninit,
                                    ),
                                )?;
                            }
                            _ => {
                                return Err(Error::new_spanned(
                                    attr,
                                    "Invalid argument. `arch`, `comp` and `mutable` have no \
                                     effect unless both `arch` and `comp` are supplied.",
                                ));
                            }
                        }
                    }
                    ArgOpt::EntityCreator(_, opts) => {
                        let arch = opts.find_one(
                            |opt| option_match!(opt, EntityCreatorArgOpt::Arch(_, ty) => ty),
                        )?;
                        let no_partition = opts
                            .find_one(
                                |opt| option_match!(opt, EntityCreatorArgOpt::NoPartition => &()),
                            )?
                            .is_some();

                        match arch {
                            Some((_, arch)) => {
                                set_arg_type(
                                    &mut arg_type,
                                    arg.name,
                                    ArgType::EntityCreator { arch: arch.clone(), no_partition },
                                )?;
                            }
                            None => {
                                set_partial_arg_type(
                                    &mut arg_type,
                                    arg.name,
                                    entity_creator_partial_builder(no_partition),
                                )?;
                            }
                        }
                    }
                    ArgOpt::EntityDeleter(_, opts) => {
                        let arch = opts.find_one(
                            |opt| option_match!(opt, EntityDeleterArgOpt::Arch(_, ty) => ty),
                        )?;

                        match arch {
                            Some((_, arch)) => {
                                set_arg_type(
                                    &mut arg_type,
                                    arg.name,
                                    ArgType::EntityDeleter { arch: arch.clone() },
                                )?;
                            }
                            None => {
                                set_partial_arg_type(
                                    &mut arg_type,
                                    arg.name,
                                    entity_deleter_partial_builder(),
                                )?;
                            }
                        }
                    }
                }
            }
        }

        let arg_type = match arg_type {
            MaybeArgType::Some(arg_type) => arg_type,
            arg_type => {
                const USAGE_INFERENCE_ERROR: &str =
                    "Cannot infer parameter usage. Specify explicitly with `#[dynec(...)]`, or \
                     use the form `impl system::(Read|Write)(Simple|Isotope)<Arch, Comp>` or \
                     `impl system::Entity(Creator|Deleter)`.";

                let impl_ty = match &*param.ty {
                    syn::Type::ImplTrait(ty) => ty,
                    _ => return Err(Error::new_spanned(&param, USAGE_INFERENCE_ERROR)),
                };

                if impl_ty.bounds.len() != 1 {
                    return Err(Error::new_spanned(&impl_ty.bounds, USAGE_INFERENCE_ERROR));
                }

                let bound = match impl_ty.bounds.first().expect("bounds.len() == 1") {
                    syn::TypeParamBound::Trait(bound) => bound,
                    bound => return Err(Error::new_spanned(bound, USAGE_INFERENCE_ERROR)),
                };

                let trait_name = bound.path.segments.last().expect("path should not be empty");
                let trait_name_string = trait_name.ident.to_string();

                let builder = match arg_type {
                    MaybeArgType::Partial { builder, .. } => builder,
                    MaybeArgType::None => match trait_name_string.as_str() {
                        "ReadSimple" | "WriteSimple" => simple_partial_builder(false, Vec::new()),
                        "ReadIsotope" | "WriteIsotope" => {
                            isotope_partial_builder(false, None, Vec::new())
                        }
                        "EntityCreator" => entity_creator_partial_builder(false),
                        "EntityDeleter" => entity_deleter_partial_builder(),
                        _ => return Err(Error::new_spanned(trait_name, USAGE_INFERENCE_ERROR)),
                    },
                    _ => unreachable!(),
                };

                let type_args = match &trait_name.arguments {
                    syn::PathArguments::AngleBracketed(args) => args,
                    _ => {
                        return Err(Error::new_spanned(
                            &trait_name.arguments,
                            USAGE_INFERENCE_ERROR,
                        ))
                    }
                };
                let types: Vec<&syn::Type> = type_args
                    .args
                    .iter()
                    .map(|arg| match arg {
                        syn::GenericArgument::Type(ty) => Ok(ty),
                        _ => Err(Error::new_spanned(arg, USAGE_INFERENCE_ERROR)),
                    })
                    .collect::<Result<_>>()?;
                builder(&trait_name_string, &types, type_args.span())?
            }
        };

        let run_arg = match arg_type {
            ArgType::Local { default, has_entity } => {
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
                local_state_entity_attrs.push(has_entity.then(|| quote!(#[entity])));

                if let Some(default) = default {
                    initial_state_field_idents.push(field_name.clone());
                    initial_state_field_defaults.push(default);
                } else {
                    param_state_field_idents.push(field_name.clone());
                    param_state_field_tys.push(syn::Type::clone(param_ty));
                }

                quote!(&mut self.#field_name)
            }
            ArgType::Global { thread_local, maybe_uninit } => {
                if thread_local && !system_thread_local {
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
                            "#[local] and #[param] can only be used on reference type parameters",
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
            ArgType::Isotope { mutable, arch, comp, discrim, maybe_uninit } => {
                let discrim_vec = discrim.as_ref().map(|discrim| {
                    quote!({
                        let __iter = ::std::iter::IntoIterator::into_iter(#discrim);
                        let __iter = ::std::iter::Iterator::map(__iter, |d| {
                            let _: &(<#comp as #crate_name::comp::Isotope<#arch>>::Discrim) = &d; // type check
                            #crate_name::comp::Discrim::into_usize(d)
                        });
                        ::std::iter::Iterator::collect::<::std::vec::Vec<_>>(__iter)
                    })
                });

                let discrim_vec_option = match &discrim {
                    Some(_) => quote!(Some(#discrim_vec)),
                    None => quote!(None),
                };
                isotope_requests.push(quote! {
                    #crate_name::system::spec::IsotopeRequest::new::<#arch, #comp>(#discrim_vec_option, #mutable)
                        #(.maybe_uninit::<#maybe_uninit>())*
                });

                let discrim_field = discrim.map(|_| {
                    let discrim_ident =
                        quote::format_ident!("__dynec_isotope_discrim_{}", param_index);
                    isotope_discrim_idents.push(discrim_ident.clone());
                    isotope_discrim_values.push(discrim_vec);

                    quote!(&self.#discrim_ident, )
                });

                let method_ident = match (mutable, discrim_field.is_some()) {
                    (true, true) => quote!(write_partial_isotope_storage),
                    (true, false) => quote!(write_full_isotope_storage),
                    (false, true) => quote!(read_partial_isotope_storage),
                    (false, false) => quote!(read_full_isotope_storage),
                };

                quote!(components.#method_ident::<#arch, #comp>(#discrim_field))
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

    let fn_body = &*input.block;

    let input_args: Vec<_> = input.sig.inputs.iter().collect();

    let input_proxy_args: Vec<_> =
        (0..input.sig.inputs.len()).map(|i| quote::format_ident!("arg_{}", i)).collect();

    let destructure_local_states = quote! {
        #[allow(unused_variables, clippy::unused_unit)]
        let (#(#local_state_field_pats,)*) = {
            let &Self {
                #(ref #local_state_field_idents,)*
                #(#isotope_discrim_idents: _,)*
            } = self;
            (#(#local_state_field_idents,)*)
        };
    };

    let (system_trait, system_run_params) = match system_thread_local {
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

    let mut local_state_struct = syn::parse2(quote! {
        #[allow(non_camel_case_types)]
        struct __dynec_local_state {
            #(#local_state_entity_attrs #local_state_field_idents: #local_state_field_tys,)*
            #(#isotope_discrim_idents: Vec<usize>,)*
        }
    })?;
    let local_state_impl_entity_ref = entity_ref::entity_ref(
        &mut local_state_struct,
        crate_name.clone(),
        quote! {
            this_field_references_an_entity_so_it_should_use_dynec_param_entity_or_dynec_local_entity
        },
    )?;

    let output = quote! {
        #(#[#other_attrs])*
        #[derive(Clone, Copy)]
        #[allow(non_camel_case_types)]
        #vis struct #ident;

        const _: () = {
            impl #ident {
                #vis fn build(
                    &self,
                    #(#param_state_field_idents: #param_state_field_tys,)*
                ) -> impl #crate_name::system::#system_trait {
                    __dynec_local_state {
                        #(#param_state_field_idents,)*
                        #(#initial_state_field_idents: #initial_state_field_defaults,)*
                        #(#isotope_discrim_idents: #isotope_discrim_values,)*
                    }
                }
            }

            #local_state_struct
            #local_state_impl_entity_ref

            fn __dynec_original(#(#input_args),*) {
                #fn_body
            }

            impl #ident {
                #vis fn call(#(#input_proxy_args: #input_types),*) {
                    __dynec_original(#(#input_proxy_args),*)
                }
            }
            /*
            // TODO: can we figure out another way to let user call the original function directly?
            impl ::std::ops::Deref for #ident {
                type Target = fn(#(#input_types),*);

                fn deref(&self) -> &Self::Target {
                    &(__dynec_original as fn(#(#input_types),*))
                }
            }
            */

            impl #crate_name::system::Descriptor for __dynec_local_state {
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

            impl #crate_name::system::#system_trait for __dynec_local_state {
                fn run(&mut self, #system_run_params) {
                    let offline_buffer = ::std::cell::RefCell::new(offline_buffer);

                    __dynec_original(#(#system_run_args),*)
                }

                fn as_descriptor_mut(&mut self) -> &mut dyn #crate_name::system::Descriptor { self }
            }
        };
    };
    // println!("{}", &output);
    Ok(output)
}

enum FnOpt {
    DynecAs(syn::token::Paren, TokenStream),
    ThreadLocal,
    Before(syn::token::Paren, Punctuated<syn::Expr, syn::Token![,]>),
    After(syn::token::Paren, Punctuated<syn::Expr, syn::Token![,]>),
    Name(syn::Token![=], Box<syn::Expr>),
    MaybeUninit(syn::token::Paren, Punctuated<syn::Type, syn::Token![,]>),
}

impl Parse for Named<FnOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "dynec_as" => {
                let inner;
                let paren = syn::parenthesized!(inner in input);
                let args = inner.parse()?;
                FnOpt::DynecAs(paren, args)
            }
            "thread_local" => FnOpt::ThreadLocal,
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
            "maybe_uninit" => parse_maybe_uninit(input, FnOpt::MaybeUninit)?,
            _ => return Err(Error::new_spanned(&name, "Unknown attribute")),
        };

        Ok(Named { name, value })
    }
}

enum ArgOpt {
    Param(Option<syn::token::Paren>, Attr<ParamArgOpt>),
    Local(Option<syn::token::Paren>, Attr<LocalArgOpt>),
    Global(Option<syn::token::Paren>, Attr<GlobalArgOpt>),
    Simple(Option<syn::token::Paren>, Attr<SimpleArgOpt>),
    Isotope(Option<syn::token::Paren>, Attr<IsotopeArgOpt>),
    EntityCreator(Option<syn::token::Paren>, Attr<EntityCreatorArgOpt>),
    EntityDeleter(Option<syn::token::Paren>, Attr<EntityDeleterArgOpt>),
}

impl Parse for Named<ArgOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "param" => parse_opt_list(input, ArgOpt::Param)?,
            "local" => parse_opt_list(input, ArgOpt::Local)?,
            "global" => parse_opt_list(input, ArgOpt::Global)?,
            "simple" => parse_opt_list(input, ArgOpt::Simple)?,
            "isotope" => parse_opt_list(input, ArgOpt::Isotope)?,
            "entity_creator" => parse_opt_list(input, ArgOpt::EntityCreator)?,
            "entity_deleter" => parse_opt_list(input, ArgOpt::EntityDeleter)?,
            _ => return Err(Error::new_spanned(&name, "Unknown attribute")),
        };
        Ok(Named { name, value })
    }
}

fn parse_opt_list<T>(
    input: ParseStream,
    arg_opt_variant: fn(Option<syn::token::Paren>, Attr<T>) -> ArgOpt,
) -> Result<ArgOpt>
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

enum LocalArgOpt {
    HasEntity,
    Initial(syn::Token![=], Box<syn::Expr>),
}

impl Parse for Named<LocalArgOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "entity" => LocalArgOpt::HasEntity,
            "initial" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let expr = input.parse::<syn::Expr>()?;
                LocalArgOpt::Initial(eq, Box::new(expr))
            }
            _ => return Err(Error::new_spanned(&name, "Unknown option for #[dynec(local)]")),
        };
        Ok(Named { name, value })
    }
}

enum ParamArgOpt {
    HasEntity,
}

impl Parse for Named<ParamArgOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "entity" => ParamArgOpt::HasEntity,
            _ => return Err(Error::new_spanned(&name, "Unknown option for #[dynec(param)]")),
        };
        Ok(Named { name, value })
    }
}

enum GlobalArgOpt {
    ThreadLocal,
    MaybeUninit(syn::token::Paren, Punctuated<syn::Type, syn::Token![,]>),
}

impl Parse for Named<GlobalArgOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "thread_local" => GlobalArgOpt::ThreadLocal,
            "maybe_uninit" => parse_maybe_uninit(input, GlobalArgOpt::MaybeUninit)?,
            _ => return Err(Error::new_spanned(&name, "Unknown option for #[dynec(global)]")),
        };
        Ok(Named { name, value })
    }
}

enum SimpleArgOpt {
    Mutable,
    Arch(syn::Token![=], Box<syn::Type>),
    Comp(syn::Token![=], Box<syn::Type>),
    MaybeUninit(syn::token::Paren, Punctuated<syn::Type, syn::Token![,]>),
}

impl Parse for Named<SimpleArgOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "mut" => SimpleArgOpt::Mutable,
            "arch" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                SimpleArgOpt::Arch(eq, Box::new(ty))
            }
            "comp" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                SimpleArgOpt::Comp(eq, Box::new(ty))
            }
            "maybe_uninit" => parse_maybe_uninit(input, SimpleArgOpt::MaybeUninit)?,
            _ => return Err(Error::new_spanned(&name, "Unknown option for #[dynec(simple)]")),
        };
        Ok(Named { name, value })
    }
}

enum IsotopeArgOpt {
    Mutable,
    Arch(syn::Token![=], Box<syn::Type>),
    Comp(syn::Token![=], Box<syn::Type>),
    Discrim(syn::Token![=], Box<syn::Expr>),
    MaybeUninit(syn::token::Paren, Punctuated<syn::Type, syn::Token![,]>),
}

impl Parse for Named<IsotopeArgOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "mut" => IsotopeArgOpt::Mutable,
            "arch" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                IsotopeArgOpt::Arch(eq, Box::new(ty))
            }
            "comp" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                IsotopeArgOpt::Comp(eq, Box::new(ty))
            }
            "discrim" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let discrim = input.parse::<syn::Expr>()?;
                IsotopeArgOpt::Discrim(eq, Box::new(discrim))
            }
            "maybe_uninit" => parse_maybe_uninit(input, IsotopeArgOpt::MaybeUninit)?,
            _ => return Err(Error::new_spanned(&name, "Unknown option for #[dynec(isotope)]")),
        };
        Ok(Named { name, value })
    }
}

enum EntityCreatorArgOpt {
    Arch(syn::Token![=], Box<syn::Type>),
    NoPartition,
}

impl Parse for Named<EntityCreatorArgOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "arch" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                EntityCreatorArgOpt::Arch(eq, Box::new(ty))
            }
            "no_partition" => EntityCreatorArgOpt::NoPartition,
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

enum EntityDeleterArgOpt {
    Arch(syn::Token![=], Box<syn::Type>),
}

impl Parse for Named<EntityDeleterArgOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "arch" => {
                let eq = input.parse::<syn::Token![=]>()?;
                let ty = input.parse::<syn::Type>()?;
                EntityDeleterArgOpt::Arch(eq, Box::new(ty))
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

fn parse_maybe_uninit<T>(
    input: ParseStream,
    constructor: fn(syn::token::Paren, Punctuated<syn::Type, syn::Token![,]>) -> T,
) -> Result<T> {
    let inner;
    let paren = syn::parenthesized!(inner in input);
    let punctuated = Punctuated::parse_terminated(&inner)?;
    Ok(constructor(paren, punctuated))
}
