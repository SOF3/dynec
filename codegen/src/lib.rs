#![feature(drain_filter)]

use proc_macro::TokenStream;

mod accessors;
mod archetype;
mod comp;
mod comps;
mod discrim;
mod entity_ref;
mod global;
mod system;
mod util;

#[proc_macro]
pub fn accessors(input: TokenStream) -> TokenStream {
    accessors::imp(input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

#[proc_macro]
pub fn archetype(input: TokenStream) -> TokenStream {
    archetype::imp(input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

#[proc_macro_attribute]
pub fn comp(args: TokenStream, input: TokenStream) -> TokenStream {
    comp::imp(args.into(), input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

#[proc_macro]
pub fn comps(input: TokenStream) -> TokenStream {
    comps::imp(input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

#[proc_macro_attribute]
pub fn global(args: TokenStream, input: TokenStream) -> TokenStream {
    global::imp(args.into(), input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

#[proc_macro_derive(EntityRef, attributes(entity, not_entity))]
pub fn entity_ref(input: TokenStream) -> TokenStream {
    entity_ref::derive(input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

#[proc_macro_derive(Discrim, attributes(dynec))]
pub fn discrim(input: TokenStream) -> TokenStream {
    discrim::derive(input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

#[proc_macro_attribute]
pub fn system(args: TokenStream, input: TokenStream) -> TokenStream {
    system::imp(args.into(), input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}
