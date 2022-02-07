extern crate proc_macro;

mod archetype;
mod component;
mod system;
mod util;

#[proc_macro_derive(Component, attributes(component))]
pub fn component(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    component::imp(input.into())
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_derive(Archetype)]
pub fn archetype(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    archetype::imp(input.into())
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn system(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    system::imp(args.into(), input.into(), false)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn subroutine(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    system::imp(args.into(), input.into(), true)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
