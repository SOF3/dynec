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

#[cfg(test)]
mod archetype_tests {}

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
/// use dynec::component;
///
/// dynec::archetype!(Foo; Bar);
///
/// #[component(of = Foo, of = Bar, init = || Qux(1), finalizer)]
/// struct Qux(i32);
///
/// static_assertions::assert_impl_all!(Qux: component::Simple<Foo>, component::Simple<Bar>);
/// assert!(matches!(<Qux as component::Simple<Foo>>::PRESENCE, component::SimplePresence::Optional));
/// assert!(<Qux as component::Simple<Bar>>::IS_FINALIZER);
///
/// #[derive(Debug, Clone, Copy)]
/// struct Id(usize);
/// impl component::Discrim for Id {
///     fn from_usize(usize: usize) -> Self { Self(usize) }
///     fn to_usize(self) -> usize { self.0 }
/// }
///
/// #[component(of = Foo, isotope = Id, init = Corge::make/0)]
/// struct Corge(i32);
///
/// impl Corge {
///     fn make() -> Self { Self(1) }
/// }
/// ```
#[doc(inline)]
pub use dynec_codegen::component;

#[cfg(test)]
mod component_tests {}

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

#[cfg(test)]
mod components_tests {}

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

#[cfg(test)]
mod global_tests {}

/// Converts a function into a system.
///
/// This macro converts the function into a unit struct with the same name
/// that implements [`system::Spec`](crate::system::Spec).
/// The unit struct also derefs to a function pointer,
/// so it is still possible to call the function directly (mainly useful in unit tests)
/// without any change in the signature.
/// However it is not recommended to call this function directly in production code.
///
/// # Arguments
/// ## `name = $expr`
/// Sets the [name](crate::system::Spec::debug_name) of the system to `$expr`.
/// By default, the name is `concat!(module_path!(), "::", $function_identifier)`.
///
/// The `$expr` can read the local states of the system directly.
/// Since the debug name is only used for display purposes,
/// it is allowed (although confusing to the programmer) to use mutable states in the name.
///
/// ```
/// use dynec::system;
///
/// #[dynec::global(initial = Title("hello world"))]
/// struct Title(&'static str);
///
/// #[system(
///     name = format!("simulate[one = {}, two = {}]", counter_one, counter_two),
/// )]
/// fn simulate(
///     #[dynec(local = 0)] counter_one: &mut u16,
///     #[dynec(param)] counter_two: &mut i64,
///     #[dynec(global)] title: &mut Title,
/// ) {
///     *counter_one += 1u16;
///     *counter_two += 3i64;
///
///     if *counter_two == 5 {
///         title.0 = "changed";
///     }
/// }
///
/// {
///     // We can call the function directly in unit tests.
///
///     let mut counter_one = 0u16;
///     let mut counter_two = 2i64;
///     let mut title = Title("original");
///
///     simulate(&mut counter_one, &mut counter_two, &mut title);
///
///     assert_eq!(counter_one, 1u16);
///     assert_eq!(counter_two, 5i64);
///     assert_eq!(title.0, "changed");
/// }
///
/// let spec = simulate.build(7i64);
/// assert_eq!(system::Spec::debug_name(&spec), "simulate[one = 0, two = 7]");
/// ```
#[doc(inline)]
pub use dynec_codegen::system;

#[cfg(test)]
mod system_tests {
    #[test]
    fn test_system_name() {
        #[super::system(dynec_as(crate))]
        fn simulate(
            #[dynec(local = 0)] counter_one: &mut u16,
            #[dynec(param)] counter_two: &mut i64,
        ) {
            *counter_one += 1u16;
            *counter_two += 3i64;
        }

        let spec = simulate.build(2i64);
        {
            use crate::system::Spec;
            assert_eq!(spec.debug_name(), "dynec::macro_docs::system_tests::simulate");
        }
    }
}

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

#[cfg(test)]
mod entity_ref_tests {}
