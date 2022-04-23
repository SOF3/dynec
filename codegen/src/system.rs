use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Error, Result};

use crate::util::{self, Attr, Named};

pub(crate) fn imp(args: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let mut input: syn::ItemFn = syn::parse2(input)?;
    let ident = &input.sig.ident;
    let vis = &input.vis;
    let other_attrs = &input.attrs;

    let mut deps = Vec::new();

    let mut name = quote!(concat!(module_path!(), "::", stringify!(#ident)));

    let mut crate_name = quote!(::dynec);

    if !args.is_empty() {
        let args = syn::parse2::<Attr<FnOpt>>(args)?;

        if let Some((_, ts)) = args.find_one(|opt| match opt {
            FnOpt::DynecAs(_, ts) => Some(ts),
            _ => None,
        })? {
            crate_name = ts.clone();
        }

        for named in &args.items {
            match &named.value {
                FnOpt::DynecAs(_, _) => {} // already handled
                FnOpt::Before(_, inputs) => {
                    for dep in inputs {
                        deps.push(quote!(#crate_name::system::spec::Dependency::Before(Box::new(#dep as Box<dyn #crate_name::system::Partition>))));
                    }
                }
                FnOpt::After(_, inputs) => {
                    for dep in inputs {
                        deps.push(quote!(#crate_name::system::spec::Dependency::After(Box::new(#dep as Box<dyn #crate_name::system::Partition>))));
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

    let mut local_state_field_idents = Vec::new();
    let mut local_state_field_tys = Vec::new();

    let mut param_state_field_idents = Vec::new();
    let mut param_state_field_tys = Vec::new();

    let mut initial_state_field_idents = Vec::new();
    let mut initial_state_field_defaults = Vec::new();

    let mut input_types = Vec::new();

    let mut global_requests = Vec::new();

    for (param_index, param) in input.sig.inputs.iter_mut().enumerate() {
        let param = match param {
            syn::FnArg::Receiver(receiver) => {
                return Err(Error::new_spanned(receiver, "system funcions must not be a method"))
            }
            syn::FnArg::Typed(typed) => typed,
        };

        input_types.push(syn::Type::clone(&param.ty));

        enum ArgType {
            Local { default: Option<Box<syn::Expr>> },
            Global { thread_local: bool },
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

        for attr in util::slow_drain_filter(&mut param.attrs, |attr| attr.path.is_ident("dynec")) {
            let arg_attr = attr.parse_args::<Attr<ArgOpt>>()?;

            for arg in arg_attr.items {
                match arg.value {
                    ArgOpt::Param => {
                        set_arg_type(&mut arg_type, arg.name, ArgType::Local { default: None })?;
                    }
                    ArgOpt::Local(_, default) => {
                        set_arg_type(
                            &mut arg_type,
                            arg.name,
                            ArgType::Local { default: Some(default) },
                        )?;
                    }
                    ArgOpt::Global(_, global_opts) => {
                        let thread_local = global_opts
                            .find_one(|opt| match opt {
                                GlobalArgOpt::ThreadLocal => Some(&()),
                                _ => None,
                            })?
                            .is_some();
                        set_arg_type(&mut arg_type, arg.name, ArgType::Global { thread_local })?;
                    }
                }
            }
        }

        let arg_type = match arg_type {
            Some(arg_type) => arg_type,
            None => {
                todo!("detect arg type")
            }
        };

        match arg_type.value {
            ArgType::Local { default } => {
                let field_name = match &*param.pat {
                    syn::Pat::Ident(ident) => ident.ident.clone(),
                    _ => quote::format_ident!("__dynec_arg_{}", param_index),
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
                local_state_field_tys.push(syn::Type::clone(param_ty));

                if let Some(default) = default {
                    initial_state_field_idents.push(field_name);
                    initial_state_field_defaults.push(default);
                } else {
                    param_state_field_idents.push(field_name);
                    param_state_field_tys.push(syn::Type::clone(param_ty));
                }
            }
            ArgType::Global { thread_local } => {
                let is_sync = !thread_local;

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
                    f(#crate_name::system::spec::GlobalRequest {
                        global: ::std::any::TypeId::of::<#param_ty>(),
                        mutable: #mutable,
                        sync: #is_sync,
                    });
                });
            }
        }
    }

    let fn_body = &*input.block;

    let input_args: Vec<_> = input.sig.inputs.iter().collect();

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
                ) -> impl #crate_name::system::Spec {
                    __dynec_local_state {
                        #(#param_state_field_idents,)*
                        #(#initial_state_field_idents: #initial_state_field_defaults,)*
                    }
                }
            }

            #[allow(non_camel_case_types)]
            struct __dynec_local_state {
                #(#local_state_field_idents: #local_state_field_tys,)*
            }

            fn __dynec_original(#(#input_args),*) {
                #fn_body
            }

            impl ::std::ops::Deref for #ident {
                type Target = fn(#(#input_types),*);

                fn deref(&self) -> &Self::Target {
                    &(__dynec_original as fn(#(#input_types),*))
                }
            }

            impl #crate_name::system::Spec for __dynec_local_state {
                fn debug_name(&self) -> ::std::string::String {
                    let &Self {
                        #(ref #local_state_field_idents,)*
                    } = self;
                    ::std::string::String::from(#name)
                }

                fn for_each_dependency(&self, f: &mut dyn FnMut(#crate_name::system::spec::Dependency)) {
                    todo!()
                }

                fn for_each_global_request(&self, f: &mut dyn FnMut(#crate_name::system::spec::GlobalRequest)) {
                    #(#global_requests)*
                }

                fn for_each_simple_request(&self, f: &mut dyn FnMut(#crate_name::system::spec::SimpleRequest)) {
                    todo!()
                }

                fn for_each_isotope_request(&self, f: &mut dyn FnMut(#crate_name::system::spec::IsotopeRequest)) {
                    todo!()
                }

                fn run(&mut self) {
                    todo!()
                }
            }
        };
    };
    // println!("{}", &output);
    Ok(output)
}

enum FnOpt {
    DynecAs(syn::token::Paren, TokenStream),
    Before(syn::token::Paren, Punctuated<syn::Expr, syn::Token![,]>),
    After(syn::token::Paren, Punctuated<syn::Expr, syn::Token![,]>),
    Name(syn::Token![=], Box<syn::Expr>),
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
    Global(Option<syn::token::Paren>, Attr<GlobalArgOpt>),
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
            "global" => {
                let mut paren = None;
                let mut opts = Attr::default();

                if input.peek(syn::token::Paren) {
                    let inner;
                    paren = Some(syn::parenthesized!(inner in input));
                    opts = inner.parse::<Attr<GlobalArgOpt>>()?;
                }

                ArgOpt::Global(paren, opts)
            }
            _ => return Err(Error::new_spanned(&name, "Unknown attribute")),
        };
        Ok(Named { name, value })
    }
}

enum GlobalArgOpt {
    ThreadLocal,
}

impl Parse for Named<GlobalArgOpt> {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let name_string = name.to_string();

        let value = match name_string.as_str() {
            "thread_local" => GlobalArgOpt::ThreadLocal,
            _ => return Err(Error::new_spanned(&name, "Unknown option for #[dynec(global)]")),
        };
        Ok(Named { name, value })
    }
}
