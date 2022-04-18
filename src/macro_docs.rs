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
/// Derives a [`component::Simple`](crate::component::Simple)
/// or [`component::Isotope`](crate::component::Isotope) implementation for the applied type.
/// This macro does not modify the input other than stripping attributes.
///
/// This macro calls [`EntityRef`] implicitly.
/// Fields that reference entities should be annotated with `#[entity]`.
///
/// # Arguments
/// ## `of = $ty`
/// Implements the applied type as a component of the archetype `$ty`.
///
/// ## `isotope = $ty`
/// Indicates that the applied type is an [isotope component](crate::component::Isotope)
/// with [discriminant](crate::component::Isotope::Discrim) of type `$ty`.
/// Indicates that the type is an isotope component (with discriminant type
/// `$ty`) instead of a simple component.
///
/// ## `required`
/// Indicates that the component must be [present](crate::component::SimplePresence)
/// for an entity of its archetype any time as long as the entity is created andnot destroyed.
///
/// This argument is exclusive with `isotope`,
/// because isotopes are always unset for an unknown discriminant.
///
/// ## `finalizer`
/// Indicates that the component is a [finalizer](crate::component::Simple::IS_FINALIZER).
///
/// ## `init`
/// Provides an initializer for the component
/// that gets called when the entity was created without this component.
/// This initializer should be either a closure with explicit parameter types,
/// or a function reference with arity in the form `path/arity` (e.g. `count/1`).
///
/// # Example
/// ```
/// dynec::archetype!(Foo);
///
/// #[dynec::component(of = Foo)]
/// struct Bar(i32);
/// ```
#[doc(inline)]
pub use dynec_codegen::component;
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
/// Derives a [`Global`](crate::Global) implementation for the applied type.
/// This macro does not modify the input other than stripping attributes.
///
/// The `initial` argument can be used to specify an initial value for the global.
/// If `initial` is given without a value, the global will be initialized to `Default::default()`.
///
/// This macro calls [`EntityRef`] implicitly.
/// Fields that reference entities should be annotated with `#[entity]`.
///
/// # Example
/// ```
/// #[dynec::global(initial = Foo(5))]
/// struct Foo(i32);
///
/// #[dynec::global(initial)]
/// #[derive(Default)]
/// struct Bar(i32);
/// ```
///
/// ```should_panic
/// #[dynec::global]
/// struct Qux(i32);
///
/// todo!("Verify that this test case panics when `Qux` is used in a system without init")
/// ```
#[doc(inline)]
pub use dynec_codegen::global;
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
/// Add the `#[entity]` attribute to fields that implement `[crate::entity::Referrer]`,
/// then the generated implementation will delegate to these fields.
///
/// This derive macro is automatically called in [`component`] and [`global`].
/// It should only be called explicitly if the type is not a component or global,
/// e.g. if it is a type included in a [``]
///
/// # Example
/// ```
/// dynec::archetype!(Foo);
///
/// #[derive(dynec::EntityRef)]
/// struct Enum {
///     #[entity]
///     entity: dynec::Entity<Foo>,
/// }
/// ```
#[doc(inline)]
pub use dynec_codegen::EntityRef;
