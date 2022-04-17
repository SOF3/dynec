/// Declares archetypes.
///
/// # Example
/// ```
/// dynec::archetype! {
///     /// This is an example archetype.
///     /// We can document it and apply attributes on it.
///     #[allow(dead_code)]
///     pub Foo;
///
///     /// Multiple archetypes can be declared in the same block
///     /// separated by semicolons.
///     /// The trailing semicolon is optional.
///     pub(crate) Bar;
/// }
///
/// static_assertions::assert_impl_all!(Foo: dynec::Archetype);
/// static_assertions::assert_impl_all!(Bar: dynec::Archetype);
/// ```
///
/// Since documentation, attributes, visibility and the trailing semicolon are optional,
/// private undocumented archetypes can be declared in a single line as well:
/// ```
/// dynec::archetype!(Foo);
/// static_assertions::assert_impl_all!(Foo: dynec::Archetype);
/// ```
#[doc(inline)]
pub use dynec_codegen::archetype;
/// Creates a map of components for a given archetype.
///
/// # Example
/// ```
/// dynec::archetype!(Foo);
/// let empty = dynec::components![Foo =>];
/// assert_eq!(empty.len(), 0);
/// ```
#[doc(inline)]
pub use dynec_codegen::components;

/// Derives a [`crate::component::Simple`]/[`crate::component::Isotope`]
/// implementation for the given type.
/// This macro does not modify the input other than stripping attributes.
#[doc(inline)]
pub use dynec_codegen::component;

/// Converts a function into a system.
///
/// This macro converts the function into a struct that derefs to a function pointer,
/// so it is still possible to call the function directly in unit tests.
/// However it is not recommended to call the converted struct directly in production code.
#[doc(inline)]
pub use dynec_codegen::system;

/// Derives a [`crate::entity::Referrer`] implementation for the type.
///
/// The generated implementation does not visit any fields by default.
/// Add the `#[has_ref]` attribute to fields that implement `[crate::entity::Referrer]`,
/// then the generated implementation will delegate to these fields.
///
/// This derive macro is automatically called in [`component`] and [`Global`].
/// It should only be called explicitly if the type is not a component or global,
/// e.g. if it is a type included in a [``]
///
/// # Example
/// ```
/// dynec::archetype!(Foo);
///
/// #[derive(dynec::HasRef)]
/// struct Enum {
///     #[has_ref] entity: dynec::Entity<Foo>,
/// }
/// ```
#[doc(inline)]
pub use dynec_codegen::HasRef;
