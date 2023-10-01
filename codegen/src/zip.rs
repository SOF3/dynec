use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;

use crate::util::Result;

pub(crate) fn imp(input: TokenStream) -> Result<TokenStream> {
    let Inputs(inputs) = syn::parse2(input)?;

    let mut output = TokenStream::new();

    for Input { crate_name, meta, vis, ident, fields, .. } in inputs {
        let crate_name = match crate_name {
            Some((_, crate_name)) => crate_name,
            None => quote!(::dynec),
        };

        let field_ty: Vec<_> =
            (0..fields.len()).map(|i| quote::format_ident!("__T{}", i)).collect();
        let field_meta: Vec<_> = fields
            .iter()
            .map(|field| {
                let meta = &field.meta;
                quote!(#(#meta)*)
            })
            .collect();
        let field_vis: Vec<_> = fields.iter().map(|field| &field.vis).collect();
        let field_ident: Vec<_> = fields.iter().map(|field| &field.ident).collect();

        let item = quote! {
            #(#meta)*
            #vis struct #ident<#(#field_ty),*> {
                #(#field_meta #field_vis #field_ident: #field_ty,)*
            }

            impl<__Arch: #crate_name::Archetype, #(#field_ty),*>
                #crate_name::system::IntoZip<__Arch>
                for #ident<#(#field_ty,)*>
            where
                #(#field_ty: #crate_name::system::IntoZip<__Arch>,)*
            {
                type IntoZip = #ident<#(<#field_ty as #crate_name::system::IntoZip<__Arch>>::IntoZip,)*>;

                fn into_zip(self) -> Self::IntoZip {
                    let #ident{#(#field_ident,)*} = self;
                    #ident { #(
                        IntoZip::<A>::into_zip(#field_ident),
                    )* }
                }
            }

            impl<__Arch: #crate_name::Archetype, #(#field_ty),*>
                #crate_name::system::Zip<__Arch>
                for #ident<#(#field_ty,)*>
            where
                #(#field_ty: #crate_name::system::Zip<__Arch>,)*
            {
                fn split(&mut self, offset: __Arch::RawEntity) -> Self {
                    let (#(#field_ident,)*) = self;
                    #ident { #(
                        #field_ident: <#field_ty as #crate_name::system::access::Zip<__Arch>>::split(#field_ident, offset),
                    )* }
                }

                type Item = #ident<
                    #(<#field_ty as #crate_name::system::access::Zip<__Arch>>::Item,)*
                >;
                fn get_chunk<E: #crate_name::entity::TempRef<__Arch>>(self, __dynec_entity: E) -> Self::Item {
                    let Self { #(#field_ident,)* } = self;
                    let __dynec_entity = entity::TempRef::<A>::new(__dynec_entity.id());
                    #ident { #(
                        #field_ident: <#field_ty as #crate_name::system::access::Zip<__Arch>>::get(
                            #field_ident,
                            __dynec_entity,
                        ),
                    )* }
                }
            }

            impl<__Arch: #crate_name::Archetype, #(#field_ty),*>
                #crate_name::system::access::ZipChunked<__Arch>
                for #ident<#(#field_ty,)*>
            where
                #(#field_ty: #crate_name::system::access::ZipChunked<__Arch>,)*
            {
                type Chunk = #ident<
                    #(<#field_ty as #crate_name::system::access::ZipChunked<__Arch>>::Chunk,)*
                >;
                fn get_chunk(self, __dynec_chunk: #crate_name::entity::TempRefChunk<__Arch>) -> Self::Chunk {
                    let Self { #(#field_ident,)* } = self;
                    #ident { #(
                        #field_ident: <#field_ty as #crate_name::system::access::ZipChunked<__Arch>>::get_chunk(
                            #field_ident,
                            __dynec_chunk,
                        ),
                    )* }
                }
            }
        };
        output.extend(item);
    }

    Ok(output)
}

struct Inputs(Vec<Input>);

impl Parse for Inputs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut inputs = Vec::new();
        while !input.is_empty() {
            inputs.push(input.parse()?);
        }
        Ok(Self(inputs))
    }
}

struct Input {
    crate_name: Option<(syn::Token![@], TokenStream)>,
    meta:       Vec<syn::Attribute>,
    vis:        syn::Visibility,
    ident:      syn::Ident,
    _braces:    syn::token::Brace,
    fields:     Punctuated<Field, syn::Token![,]>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> Result<Self> {
        let crate_name = if input.peek(syn::Token![@]) {
            let at = input.parse()?;
            let crate_name = input.parse()?;
            Some((at, crate_name))
        } else {
            None
        };

        let meta = input.call(syn::Attribute::parse_outer)?;
        let vis = input.parse()?;
        let ident = input.parse()?;

        let inner;
        let braces = syn::braced!(inner in input);
        let fields = Punctuated::parse_terminated(&inner)?;

        Ok(Self { crate_name, meta, vis, ident, _braces: braces, fields })
    }
}

struct Field {
    meta:  Vec<syn::Attribute>,
    vis:   syn::Visibility,
    ident: syn::Ident,
}

impl Parse for Field {
    fn parse(input: ParseStream) -> Result<Self> {
        let meta = input.call(syn::Attribute::parse_outer)?;
        let vis = input.parse()?;
        let ident = input.parse()?;

        Ok(Self { meta, vis, ident })
    }
}
