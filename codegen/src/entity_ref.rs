use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Error, Result};

use crate::util;

pub(crate) fn derive(input: TokenStream) -> Result<TokenStream> {
    let mut input: syn::DeriveInput = syn::parse2(input)?;
    entity_ref(
        &mut input,
        quote!(::dynec),
        quote! {
            this_field_references_an_entity_so_it_should_have_the_entity_attribute
        },
    )
}

pub(crate) fn entity_ref(
    input: &mut syn::DeriveInput,
    crate_name: TokenStream,
    not_referrer_trait: TokenStream,
) -> Result<TokenStream> {
    let generics = util::parse_generics(input);
    let mut assert_not_referrer_types = Vec::new();

    let output = match &mut input.data {
        syn::Data::Struct(s) => {
            let mut field_values = Vec::new();
            let mut field_types = Vec::new();

            for (i, field) in s.fields.iter_mut().enumerate() {
                if drain_attr(&mut field.attrs, "entity") {
                    let i_field = syn::Index::from(i);
                    field_values.push(match &field.ident {
                        Some(ident) => quote!(self.#ident),
                        None => quote!(self.#i_field),
                    });
                    field_types.push(&field.ty);
                } else if !drain_attr(&mut field.attrs, "not_entity") {
                    assert_not_referrer_types.push(&field.ty);
                }
            }

            generics.impl_trait(
                quote!(#crate_name::entity::referrer::Referrer),
                quote! {
                    #[inline]
                    fn visit_type(arg: &mut #crate_name::entity::referrer::VisitTypeArg) {
                        if arg.mark::<Self>().is_continue() {
                            #(<#field_types as #crate_name::entity::referrer::Referrer>::visit_type(arg);)*
                        }
                    }

                    #[inline]
                    fn visit_mut<V: #crate_name::entity::referrer::VisitMutArg>(
                        &mut self,
                        arg: &mut V,
                    ) {
                        #(
                            #crate_name::entity::referrer::Referrer::visit_mut(
                                &mut #field_values,
                                arg,
                            );
                        )*
                    }
                },
            )
        }
        syn::Data::Enum(e) => {
            let mut arms = Vec::new();

            let mut all_types = Vec::new();

            for variant in &mut e.variants {
                let variant_ident = &variant.ident;

                let (pattern, fields) = match &mut variant.fields {
                    syn::Fields::Unit => (quote!(), Vec::new()),
                    syn::Fields::Unnamed(fields) => {
                        let mut field_names = Vec::new();
                        let mut entity_fields = Vec::new();

                        for (i, field) in fields.unnamed.iter_mut().enumerate() {
                            let field_name = format_ident!("field_{}", i);
                            field_names.push(field_name.clone());

                            if drain_attr(&mut field.attrs, "entity") {
                                entity_fields.push(field_name);
                                all_types.push(&field.ty);
                            } else if !drain_attr(&mut field.attrs, "not_entity") {
                                assert_not_referrer_types.push(&field.ty);
                            }
                        }

                        (quote!((#(#field_names),*)), entity_fields)
                    }
                    syn::Fields::Named(fields) => {
                        let mut entity_fields = Vec::new();

                        for field in &mut fields.named {
                            let field_name = field.ident.as_ref().expect("named fields");
                            if drain_attr(&mut field.attrs, "entity") {
                                entity_fields.push(field_name.clone());
                                all_types.push(&field.ty);
                            } else if !drain_attr(&mut field.attrs, "not_entity") {
                                assert_not_referrer_types.push(&field.ty);
                            }
                        }

                        (quote!({ #(#entity_fields,)* .. }), entity_fields)
                    }
                };

                arms.push(quote! {
                    Self::#variant_ident #pattern => {
                        #(
                            #crate_name::entity::referrer::Referrer::visit_mut(
                                #fields,
                                arg,
                            );
                        )*
                    },
                })
            }

            generics.impl_trait(
                quote!(#crate_name::entity::Referrer),
                quote! {
                    #[inline]
                    fn visit_type(arg: &mut #crate_name::entity::referrer::VisitTypeArg) {
                        if arg.mark::<Self>().is_continue() {
                            #(<#all_types as #crate_name::entity::referrer::Referrer>::visit_type(arg);)*
                        }
                    }

                    #[inline]
                    fn visit_mut<V: #crate_name::entity::referrer::VisitMutArg>(
                        &mut self,
                        arg: &mut V,
                    ) {
                        match self {
                            #(#arms)*
                        }
                    }
                },
            )
        }
        syn::Data::Union(u) => {
            return Err(Error::new_spanned(&u.union_token, "only structs and enums are supported"))
        }
    };

    let assert_not_referrer_types = assert_not_referrer_types.into_iter().map(|ty| quote::quote_spanned! { ty.span() =>
        // Copied from static_assertions: https://docs.rs/static_assertions/1.1.0/src/static_assertions/assert_impl.rs.html#265-285
        // See the linked source code for comments on the types.

        // We copy the macro source code here instead of generating code that calls it,
        // because compiler displays `Span::call_site()` by default for decl macro errors,
        // which highlights errors at the `derive` position instead of the actual field.
        // Furthermore, we rename some identifiers to generate more human-readable errors.

        const _: fn() = || {
            trait #not_referrer_trait<A> {
                fn some_item() {}
            }

            impl<T: ?Sized> #not_referrer_trait<()> for T {}

            #[allow(dead_code)]
            struct Invalid;

            impl<T: ?Sized + #crate_name::entity::Referrer> #not_referrer_trait<Invalid> for T {}

            let _ = <#ty as #not_referrer_trait<_>>::some_item;
        };
    });

    Ok(quote! {
        #output

        #(#assert_not_referrer_types)*
    })
}

fn drain_attr(vec: &mut Vec<syn::Attribute>, ident: &str) -> bool {
    match vec.iter().position(|attr| attr.path.is_ident(ident)) {
        Some(index) => {
            vec.remove(index);
            true
        }
        None => false,
    }
}
