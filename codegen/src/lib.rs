use proc_macro::TokenStream;

extern crate proc_macro;

mod archetype;
mod component;
mod components;
mod entity_ref;
mod global;
mod system;
mod util;

#[proc_macro]
pub fn archetype(input: TokenStream) -> TokenStream {
    archetype::imp(input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

#[proc_macro_attribute]
pub fn component(args: TokenStream, input: TokenStream) -> TokenStream {
    component::imp(args.into(), input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

#[proc_macro]
pub fn components(input: TokenStream) -> TokenStream {
    components::imp(input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

#[proc_macro_attribute]
pub fn global(args: TokenStream, input: TokenStream) -> TokenStream {
    global::imp(args.into(), input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

#[proc_macro_derive(EntityRef, attributes(entity))]
pub fn entity_ref(input: TokenStream) -> TokenStream {
    entity_ref::derive(input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

#[proc_macro_attribute]
pub fn system(args: TokenStream, input: TokenStream) -> TokenStream {
    system::imp(args.into(), input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}
