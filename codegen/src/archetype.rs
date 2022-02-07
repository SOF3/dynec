use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Error, Result};

pub(crate) fn imp(input: TokenStream) -> Result<TokenStream> {
    let input: syn::DeriveInput = syn::parse2(input)?;

    let ident = &input.ident;

    let data = match &input.data {
        syn::Data::Enum(data) => data,
        _ => {
            return Err(Error::new(
                Span::call_site(),
                "Archetype can only be derived from empty enums",
            ))
        }
    };

    if !data.variants.is_empty() {
        return Err(Error::new_spanned(
            &data.variants,
            "Archetype can only be derived from empty enums",
        ));
    }

    let output = quote! {
        impl ::dynec::Archetype for #ident {}
    };

    Ok(output)
}
