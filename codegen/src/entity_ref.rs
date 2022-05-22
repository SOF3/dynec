use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Error, Result};

use crate::util;

pub(crate) fn derive(input: TokenStream) -> Result<TokenStream> {
    let mut input: syn::DeriveInput = syn::parse2(input)?;
    entity_ref(&mut input, quote!(::dynec))
}

pub(crate) fn entity_ref(
    input: &mut syn::DeriveInput,
    crate_name: TokenStream,
) -> Result<TokenStream> {
    let generics = util::parse_generics(input);

    let output = match &mut input.data {
        syn::Data::Struct(s) => {
            let mut fields = Vec::new();

            for (i, field) in s.fields.iter_mut().enumerate() {
                if drain_entity_attr(&mut field.attrs) {
                    fields.push(match &field.ident {
                        Some(ident) => quote!(self.#ident),
                        None => quote!(self.#i),
                    });
                }
            }

            generics.impl_trait(
                quote!(#crate_name::entity::Referrer),
                quote! {
                    fn visit(
                        &mut self,
                        arg: &mut #crate_name::entity::ReferrerArg,
                    ) {
                        #(
                            #crate_name::entity::Referrer::visit(
                                &mut #fields,
                                &mut *arg,
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
                        let mut entity_fields = Vec::new();

                        for (i, field) in fields.unnamed.iter_mut().enumerate() {
                            let field_name = format_ident!("field_{}", i);
                            field_names.push(field_name.clone());

                            if drain_entity_attr(&mut field.attrs) {
                                entity_fields.push(field_name);
                            }
                        }

                        (quote!((#(#field_names),*)), entity_fields)
                    }
                    syn::Fields::Named(fields) => {
                        let mut entity_fields = Vec::new();

                        for field in &mut fields.named {
                            let field_name = field.ident.as_ref().expect("named fields");
                            if drain_entity_attr(&mut field.attrs) {
                                entity_fields.push(field_name.clone());
                            }
                        }

                        (quote!({ #(#entity_fields,)* .. }), entity_fields)
                    }
                };

                arms.push(quote! {
                    Self::#variant_ident #pattern => {
                        #(
                            #crate_name::entity::Referrer::visit(
                                &mut #fields,
                                &mut *arg,
                            );
                        )*
                    },
                })
            }

            generics.impl_trait(
                quote!(#crate_name::entity::Referrer),
                quote! {
                    fn visit(
                        &mut self,
                        arg: &mut #crate_name::entity::ReferrerArg,
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

fn drain_entity_attr(vec: &mut Vec<syn::Attribute>) -> bool {
    match vec.iter().position(|attr| attr.path.is_ident("entity")) {
        Some(index) => {
            vec.remove(index);
            true
        }
        None => false,
    }
}
