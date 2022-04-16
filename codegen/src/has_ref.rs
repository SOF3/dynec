use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Error, Result};

use crate::util;

pub(crate) fn derive(input: TokenStream) -> Result<TokenStream> {
    let mut input: syn::DeriveInput = syn::parse2(input)?;
    has_ref(&mut input)
}

pub(crate) fn has_ref(input: &mut syn::DeriveInput) -> Result<TokenStream> {
    let generics = util::parse_generics(input);

    let output = match &mut input.data {
        syn::Data::Struct(s) => {
            let mut fields = Vec::new();

            for (i, field) in s.fields.iter_mut().enumerate() {
                if drain_has_ref(&mut field.attrs) {
                    fields.push(match &field.ident {
                        Some(ident) => quote!(self.#ident),
                        None => quote!(self.#i),
                    });
                }
            }

            generics.impl_trait(
                quote!(::dynec::entity::Referrer),
                quote! {
                    fn visit<'s, 'f, F: FnMut(&'s mut ::dynec::entity::Raw)>(
                        &'s mut self,
                        ty: ::std::any::TypeId,
                        visitor: &'f mut F,
                    ) {
                        #(
                            ::dynec::entity::Referrer::visit(
                                &mut #fields,
                                ty,
                                visitor,
                            );
                        )*
                    }
                },
            )
        }
        syn::Data::Enum(e) => {
            let mut arms = Vec::new();

            for variant in &mut e.variants {
                let variant_ident = &variant.ident;

                let (pattern, fields) = match &mut variant.fields {
                    syn::Fields::Unit => (quote!(), Vec::new()),
                    syn::Fields::Unnamed(fields) => {
                        let mut field_names = Vec::new();
                        let mut has_ref_fields = Vec::new();

                        for (i, field) in fields.unnamed.iter_mut().enumerate() {
                            let field_name = format_ident!("field_{}", i);
                            field_names.push(field_name.clone());

                            if drain_has_ref(&mut field.attrs) {
                                has_ref_fields.push(field_name);
                            }
                        }

                        (quote!((#(#field_names),*)), has_ref_fields)
                    }
                    syn::Fields::Named(fields) => {
                        let mut has_ref_fields = Vec::new();

                        for field in &mut fields.named {
                            let field_name = field.ident.as_ref().expect("named fields");
                            if drain_has_ref(&mut field.attrs) {
                                has_ref_fields.push(field_name.clone());
                            }
                        }

                        (quote!({ #(#has_ref_fields,)* .. }), has_ref_fields)
                    }
                };

                arms.push(quote! {
                    Self::#variant_ident #pattern => {
                        #(
                            ::dynec::entity::Referrer::visit(
                                &mut #fields,
                                ty,
                                visitor,
                            );
                        )*
                    },
                })
            }

            generics.impl_trait(
                quote!(::dynec::entity::Referrer),
                quote! {
                    fn visit<'s, 'f, F: FnMut(&'s mut ::dynec::entity::Raw)>(
                        &'s mut self,
                        ty: ::std::any::TypeId,
                        visitor: &'f mut F,
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

    Ok(output)
}

fn drain_has_ref(vec: &mut Vec<syn::Attribute>) -> bool {
    match vec.iter().position(|attr| attr.path.is_ident("has_ref")) {
        Some(index) => {
            vec.remove(index);
            true
        }
        None => false,
    }
}
