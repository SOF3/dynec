use std::iter;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;

use crate::util::Result;

pub(crate) fn imp(input: TokenStream) -> Result<TokenStream> {
    let Inputs(inputs) = syn::parse2(input)?;

    let mut output = TokenStream::new();

    for Input { debug_print, crate_name, meta, vis, ident, fields, .. } in inputs {
        let debug_print = debug_print.is_some();

        if fields.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "zip structs must have at least one field",
            ));
        }

        let crate_name = match crate_name {
            Some((_, _, _, crate_name)) => crate_name,
            None => quote!(::dynec),
        };

        let field_tys: Vec<_> =
            (0..fields.len()).map(|i| quote::format_ident!("__T{}", i)).collect();
        let field_meta: Vec<_> = fields
            .iter()
            .map(|field| {
                let meta = &field.meta;
                quote!(#(#meta)*)
            })
            .collect();
        let field_vis: Vec<_> = fields.iter().map(|field| &field.vis).collect();
        let field_idents: Vec<_> = fields.iter().map(|field| &field.ident).collect();

        let first_field_ident = field_idents.first().expect("struct checked to be non empty");
        let first_field_ty = field_tys.first().expect("struct checked to be non empty");
        let mut stacked_zip = quote!(<#first_field_ty as #crate_name::system::ZipChunked::<__Arch>>::chunk_to_entities(#first_field_ident));
        let mut stacked_tuples = quote!(#first_field_ident);
        for (&field_ident, field_ty) in iter::zip(&field_idents, &field_tys).skip(1) {
            stacked_zip = quote!(::core::iter::zip(#stacked_zip, <#field_ty as #crate_name::system::ZipChunked::<__Arch>>::chunk_to_entities(#field_ident)));
            stacked_tuples = quote!((#stacked_tuples, #field_ident));
        }

        let item = quote! {
            #(#meta)*
            #vis struct #ident<#(#field_tys),*> {
                #(#field_meta #field_vis #field_idents: #field_tys,)*
            }

            impl<__Arch: #crate_name::Archetype, #(#field_tys),*>
                #crate_name::system::IntoZip<__Arch>
                for #ident<#(#field_tys,)*>
            where
                #(#field_tys: #crate_name::system::IntoZip<__Arch>,)*
            {
                type IntoZip = #ident<#(<#field_tys as #crate_name::system::IntoZip<__Arch>>::IntoZip,)*>;

                fn into_zip(self) -> Self::IntoZip {
                    let Self { #(#field_idents,)* } = self;
                    #ident { #(
                        #field_idents: <#field_tys as #crate_name::system::IntoZip<__Arch>>::into_zip(#field_idents),
                    )* }
                }
            }

            impl<__Arch: #crate_name::Archetype, #(#field_tys),*>
                #crate_name::system::Zip<__Arch>
                for #ident<#(#field_tys,)*>
            where
                #(#field_tys: #crate_name::system::Zip<__Arch>,)*
            {
                fn split(&mut self, offset: __Arch::RawEntity) -> Self {
                    let Self { #(#field_idents,)* } = self;
                    #ident { #(
                        #field_idents: <#field_tys as #crate_name::system::Zip<__Arch>>::split(#field_idents, offset),
                    )* }
                }

                type Item = #ident<
                    #(<#field_tys as #crate_name::system::Zip<__Arch>>::Item,)*
                >;
                fn get<E: #crate_name::entity::Ref<Archetype = __Arch>>(self, __dynec_entity: E) -> Self::Item {
                    let Self { #(#field_idents,)* } = self;
                    let __dynec_entity = #crate_name::entity::TempRef::<__Arch>::new(__dynec_entity.id());
                    #ident { #(
                        #field_idents: <#field_tys as #crate_name::system::Zip<__Arch>>::get(
                            #field_idents,
                            __dynec_entity,
                        ),
                    )* }
                }
            }

            impl<__Arch: #crate_name::Archetype, #(#field_tys),*>
                #crate_name::system::ZipChunked<__Arch>
                for #ident<#(#field_tys,)*>
            where
                #(#field_tys: #crate_name::system::ZipChunked<__Arch>,)*
            {
                type Chunk = #ident<
                    #(<#field_tys as #crate_name::system::ZipChunked<__Arch>>::Chunk,)*
                >;
                fn get_chunk(self, __dynec_chunk: #crate_name::entity::TempRefChunk<__Arch>) -> Self::Chunk {
                    let Self { #(#field_idents,)* } = self;
                    #ident { #(
                        #field_idents: <#field_tys as #crate_name::system::ZipChunked<__Arch>>::get_chunk(
                            #field_idents,
                            __dynec_chunk,
                        ),
                    )* }
                }

                fn chunk_to_entities(chunk: Self::Chunk) -> impl Iterator<Item = #ident<
                    #(<#field_tys as #crate_name::system::Zip<__Arch>>::Item,)*
                >> {
                    let #ident { #(#field_idents,)* } = chunk;
                    #stacked_zip.map(|#stacked_tuples| #ident { #(#field_idents,)* })
                }
            }
        };

        if debug_print {
            println!("{item}");
        }
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
    debug_print: Option<(syn::Token![@], kw::__debug_print)>,
    crate_name:  Option<(syn::Token![@], kw::dynec_as, syn::token::Paren, TokenStream)>,
    meta:        Vec<syn::Attribute>,
    vis:         syn::Visibility,
    ident:       syn::Ident,
    _braces:     syn::token::Brace,
    fields:      Punctuated<Field, syn::Token![,]>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut debug_print = None;
        let mut crate_name = None;

        while input.peek(syn::Token![@]) {
            let at = input.parse()?;
            let lh = input.lookahead1();
            if lh.peek(kw::__debug_print) {
                debug_print = Some((at, input.parse()?));
            } else if lh.peek(kw::dynec_as) {
                let kw = input.parse()?;
                let inner;
                let paren = syn::parenthesized!(inner in input);
                let crate_name_ts = inner.parse()?;
                crate_name = Some((at, kw, paren, crate_name_ts));
            } else {
                return Err(lh.error());
            }
        }

        let meta = input.call(syn::Attribute::parse_outer)?;
        let vis = input.parse()?;
        let ident = input.parse()?;

        let inner;
        let braces = syn::braced!(inner in input);
        let fields = Punctuated::parse_terminated(&inner)?;

        Ok(Self { debug_print, crate_name, meta, vis, ident, _braces: braces, fields })
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

mod kw {
    syn::custom_keyword!(dynec_as);
    syn::custom_keyword!(__debug_print);
}
