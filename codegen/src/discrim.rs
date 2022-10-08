use matches2::option_match;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::parse::Parse;
use syn::{Error, Result};

use crate::util::{Attr, Named};

const INPUT_TYPE_ERROR: &str =
    "Discrim can only be derived from single-field structs and fieldless enums";

pub(crate) fn derive(input: TokenStream) -> Result<TokenStream> {
    let input: syn::DeriveInput = syn::parse2(input)?;

    let mut args: Attr<ItemOpt> = Attr::default();
    for attr in &input.attrs {
        if attr.path.is_ident("dynec") {
            let this_args: Attr<ItemOpt> = attr.parse_args()?;
            args.items.extend(this_args.items);
        }
    }

    let crate_name = args
        .find_one(|opt| option_match!(opt, ItemOpt::DynecAs(_, crate_name) => crate_name))?
        .map_or_else(|| quote!(::dynec), |(_, crate_name)| crate_name.clone());
    let mut map =
        args.find_one(|opt| option_match!(opt, ItemOpt::Map(_, map) => map))?.map_or_else(
            || {
                syn::parse2::<Box<syn::Type>>(quote!(discrim::BoundedVecMap))
                    .expect("discrim::BoundedVecMap is a Type::Path")
            },
            |(_, map)| map.clone(),
        );
    if let syn::Type::Path(ty) = map.as_mut() {
        let last_segment =
            ty.path.segments.last_mut().expect("Type must have at least one segment");
        if last_segment.arguments.is_empty() {
            last_segment.arguments = syn::PathArguments::AngleBracketed(
                syn::parse2(quote!(<Self, T>)).expect("<Self, T> is an ABGA"),
            );
        }
    }

    let body = match &input.data {
        syn::Data::Struct(item) => {
            let (field_ref, field_ty) = match &item.fields {
                syn::Fields::Unit => {
                    return Err(Error::new_spanned(item.struct_token, INPUT_TYPE_ERROR))
                }
                syn::Fields::Named(fields) => {
                    if fields.named.len() != 1 {
                        return Err(Error::new_spanned(fields, INPUT_TYPE_ERROR));
                    }
                    let field = fields.named.first().expect("checked above");
                    let field_ident = field.ident.as_ref().expect("named field");
                    (field_ident.to_token_stream(), &field.ty)
                }
                syn::Fields::Unnamed(fields) => {
                    if fields.unnamed.len() != 1 {
                        return Err(Error::new_spanned(fields, INPUT_TYPE_ERROR));
                    }
                    let field = fields.unnamed.first().expect("checked above");
                    (quote!(0), &field.ty)
                }
            };

            quote! {
                type FullMap<T> = #map;

                fn from_usize(usize: usize) -> Self {
                    use #crate_name::_reexports::xias::Xias;

                    let transformed: #field_ty = <usize as Xias>::small_int(usize);
                    Self { #field_ref: transformed }
                }

                fn into_usize(self) -> usize {
                    use #crate_name::_reexports::xias::Xias;

                    let transformed: usize = <#field_ty as Xias>::small_int(self.#field_ref);
                    transformed
                }
            }
        }
        syn::Data::Enum(item) => {
            let num_variants = item.variants.len();

            let mut from_arms = Vec::new();
            let mut into_arms = Vec::new();

            for (ord, variant) in item.variants.iter().enumerate() {
                if !(matches!(variant.fields, syn::Fields::Unit)) {
                    return Err(Error::new_spanned(&variant.fields, INPUT_TYPE_ERROR));
                }

                let variant_ident = &variant.ident;
                from_arms.push(quote!(#ord => Self::#variant_ident));
                into_arms.push(quote!(Self::#variant_ident => #ord));
            }

            quote! {
                type FullMap<T> = discrim::ArrayMap<Self, T, #num_variants>;

                fn from_usize(usize: usize) -> Self {
                    match usize {
                        #(#from_arms,)*
                        _ => panic!("Invalid discriminant raw value"),
                    }
                }

                fn into_usize(self) -> usize {
                    match self {
                        #(#into_arms,)*
                    }
                }
            }
        }
        syn::Data::Union(item) => {
            return Err(Error::new_spanned(item.union_token, INPUT_TYPE_ERROR));
        }
    };

    let input_ident = &input.ident;

    let output = quote! {
        const _: () = {
            use #crate_name::comp::discrim;

            impl discrim::Discrim for #input_ident { #body }
        };
    };

    Ok(output)
}

enum ItemOpt {
    DynecAs(syn::token::Paren, TokenStream),
    Map(syn::Token![=], Box<syn::Type>),
}

impl Parse for Named<ItemOpt> {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let name = input.parse::<syn::Ident>()?;

        let value = match name.to_string().as_str() {
            "dynec_as" => {
                let inner;
                let paren = syn::parenthesized!(inner in input);
                let args = inner.parse()?;
                ItemOpt::DynecAs(paren, args)
            }
            "map" => {
                let eq: syn::Token![=] = input.parse()?;
                let ty: Box<syn::Type> = input.parse()?;
                ItemOpt::Map(eq, ty)
            }
            _ => return Err(Error::new_spanned(&name, format!("Unknown argument `{}`", name))),
        };

        Ok(Named { name, value })
    }
}
