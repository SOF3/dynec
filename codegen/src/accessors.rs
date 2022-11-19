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

            unsafe impl<__Arch: #crate_name::Archetype, #(#field_ty),*>
                #crate_name::system::Accessor<__Arch>
                for #ident<#(#field_ty,)*>
            where
                #(#field_ty: #crate_name::system::Accessor<__Arch>,)*
            {
                type Entity<'t> = #ident<
                    #(<#field_ty as #crate_name::system::Accessor<__Arch>>::Entity<'t>,)*
                > where Self: 't;
                unsafe fn entity<'this, 'e, 'ret>(this: &'this mut Self, entity: #crate_name::entity::TempRef<'e, __Arch>) -> Self::Entity<'ret> {
                    #ident {
                        #(#field_ident: <#field_ty as #crate_name::system::Accessor<__Arch>>::entity(
                            &mut this.#field_ident,
                            entity,
                        ),)*
                    }
                }
            }

            unsafe impl<__Arch: #crate_name::Archetype, #(#field_ty),*>
                #crate_name::system::ChunkedAccessor<__Arch>
                for #ident<#(#field_ty,)*>
            where
                #(#field_ty: #crate_name::system::ChunkedAccessor<__Arch>,)*
            {
                type Chunk<'t> = #ident<
                    #(<#field_ty as #crate_name::system::ChunkedAccessor<__Arch>>::Chunk<'t>,)*
                > where Self: 't;
                unsafe fn chunk<'this, 'e, 'ret>(this: &'this mut Self, chunk: #crate_name::entity::TempRefChunk<'e, __Arch>) -> Self::Chunk<'ret> {
                    #ident {
                        #(#field_ident: <#field_ty as #crate_name::system::ChunkedAccessor<__Arch>>::chunk(
                            &mut this.#field_ident,
                            chunk,
                        ),)*
                    }
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
