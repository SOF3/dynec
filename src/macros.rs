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

/// Derives a [`comp::Simple`](crate::comp::Simple)
/// or [`comp::Isotope`](crate::comp::Isotope) implementation for the applied type.
/// This macro does not modify the input other than stripping attributes.
///
/// This macro calls [`EntityRef`] implicitly.
/// Fields that reference entities should be annotated with `#[entity]`.
///
/// # Options
/// Options are applied behind the attribute name in the form `#[system(...)]`.
/// Multiple options are separated by commas.
///
/// ## `of = $ty`
/// Implements the applied type as a component of the archetype `$ty`.
///
/// ## `isotope = $ty`
/// Indicates that the applied type is an [isotope component](crate::comp::Isotope)
/// with [discriminant](crate::comp::Isotope::Discrim) of type `$ty`.
/// Indicates that the type is an isotope component (with discriminant type
/// `$ty`) instead of a simple component.
///
/// ## `required`
/// Indicates that the component must be [present](crate::comp::SimplePresence)
/// for an entity of its archetype any time as long as the entity is created andnot destroyed.
///
/// This argument is exclusive with `isotope`,
/// because isotopes are always unset for an unknown discriminant.
///
/// ## `finalizer`
/// Indicates that the component is a [finalizer](crate::comp::Simple::IS_FINALIZER).
///
/// ## `init`
/// Provides an initializer for the component
/// that gets called when the entity was created without this component.
/// This initializer should be either a closure with explicit parameter types,
/// or a function reference with arity in the form `path/arity` (e.g. `count/1`).
///
/// # Example
/// ```
/// use dynec::comp;
///
/// dynec::archetype!(Foo; Bar);
///
/// #[comp(of = Foo, of = Bar, init = || Qux(1), finalizer)]
/// struct Qux(i32);
///
/// static_assertions::assert_impl_all!(Qux: comp::Simple<Foo>, comp::Simple<Bar>);
/// assert!(matches!(<Qux as comp::Simple<Foo>>::PRESENCE, comp::SimplePresence::Optional));
/// assert!(<Qux as comp::Simple<Bar>>::IS_FINALIZER);
///
/// #[derive(Debug, Clone, Copy)]
/// struct Id(usize);
/// impl comp::Discrim for Id {
///     fn from_usize(usize: usize) -> Self { Self(usize) }
///     fn to_usize(self) -> usize { self.0 }
/// }
///
/// #[comp(of = Foo, isotope = Id, init = Corge::make/0)]
/// struct Corge(i32);
///
/// impl Corge {
///     fn make() -> Self { Self(1) }
/// }
/// ```
#[doc(inline)]
pub use dynec_codegen::comp;

#[cfg(test)]
mod comp_tests {}

/// Creates a map of components for a given archetype.
///
/// # Example
/// ```
/// dynec::archetype!(Foo);
/// let empty = dynec::comps![Foo =>];
/// assert_eq!(empty.len(), 0);
///
/// #[dynec::comp(of = Foo)]
/// struct Comp1;
/// #[dynec::comp(of = Foo)]
/// struct Comp2(i32);
/// #[dynec::comp(of = Foo)]
/// struct Comp3{ value: i32 };
///
/// let empty = dynec::comps![Foo => Comp1, Comp2(2), Comp3{ value: 3 }];
/// assert_eq!(empty.len(), 3);
/// ```
#[doc(inline)]
pub use dynec_codegen::comps;

#[cfg(test)]
mod comps_tests {}

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
/// todo!(
///     "Verify that this test case panics upon world build if `Qux` is used in a system and the \
///      global is not initialized"
/// )
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
/// # Options
/// Options are applied behind the attribute name in the form `#[system(...)]`.
/// Multiple options are separated by commas.
///
/// ## `name = $expr`
/// Sets the [name](crate::system::Spec::debug_name) of the system to `$expr`.
/// By default, the name is `concat!(module_path!(), "::", $function_identifier)`.
///
/// The `$expr` can read the local and param states of the system directly.
/// Since the debug name is only used for display purposes,
/// it is allowed (although confusing to the programmer) to use mutable states in the name.
/// It is unspecified whether debug messages use the initial debug name or the updated state.
///
/// ## `before($expr1, $expr2, ...)` and `after($expr1, $expr2, ...)`
/// Indicates that the system must be executed
/// before/after all [partitions](crate::system::Partition) given in the expressions.
///
/// Similar to `name`, the expressions can read local and param states dirctly.
/// However, only the expressions are only resolved once before the first run of the system,
/// so mutating states has no effect on the system schedule.
///
/// # Parameters
/// Each parameter of a system function has a special meaning:
///
/// ## Local states
/// Parameters with the attribute `#[dynec(local = xxx)]` are "local states",
/// where `xxx` is an expression that evaluates to the initial value of the state.
///
/// Local states must take the type `&T` or `&mut T`,
/// where `T` is the actual stored state.
/// The mutated state persists for each instance of the system.
///
/// Use global states instead if the local state needs to be accessed from multiple systems.
///
/// ## Param states
/// Parameters with the attribute `#[dynec(param)]` are "param states".
/// The user has to pass initial values for param states in the `.build()` method.
/// Param states behave identically to local states
/// except for different definition location of the initial value.
///
/// It is typically used to initialize systems with resources that cannot be created statically
/// (e.g. system canvas resources),
/// or to schedule multiple systems declared from the same function
/// (e.g. working on multiple discriminants of an isotope component).
///
/// ## Global states
/// Parameters with the attribute `#[dynec(global)]` are "global states".
/// Global states are shared scalar data between multiple systems.
/// See [`Global`](crate::Global) for more information.
///
/// Thread-unsafe (non-`Send + Sync`) global states must be declared as
/// `#[dynec(global(thread_local))]` to indicate that
/// the global state can only be accessed from the main thread.
/// As a result, systems that request thread-local global states
/// will only be scheduled on the main thread.
///
/// ## Simple components
/// Parameters with the attribute `#[dynec(simple(arch = Type1, comp = Type2))]`
/// are "simple components".
/// Alternatively, parameters that do not have a `#[dynec]` attribute
/// but with a type `impl ReadSimple<Type1, Type2>`/`impl WriteSimple<Type1, Type2>`
/// are also simple components
/// (where `Simple` can be qualified by any path, e.g. `system::Simple`).
/// This requests access to a [simple component](crate::comp::Simple) of type `Type2`
/// from entities of the [archetype](crate::Archetype) `Type1`,
/// exposed through a type that implements [`system::Simple`](crate::system::Simple).
///
/// With the attribute, only read access is permitted by default.
/// To allow write access, add `mut` to the `simple` option list with the first syntax.
/// No information is inferred from the type if `#[dync(simple)]`
///
/// ## Isotope components
/// Parameters with the attribute `#[dynec(isotope(arch = Type1, comp = Type2))]`
/// are "isotope components".
/// Alternatively, parameters that do not have a `#[dynec]` attribute
/// but with a type `Isotope<Type1, &Type2>` are also isotope components.
/// This requests access to an [isotope component](crate::comp::Isotope) of type `Type2`
/// from entities of the [archetype](crate::Archetype) `Type1`,
/// exposed through a type that implements [`system::Isotope`](crate::system::Isotope).
///
/// Only read access is permitted by default.
/// To allow write access, add `mut` to the `isotope` option list with the first syntax,
/// or change the component reference to `&mut` with the second syntax.
///
/// By default, all discriminants of the isotope component are requested.
/// To request only particular discriminants,
/// add `discrim = expr` to the `isotope` option list with the first syntax,
/// or add the `#[dynec(isotope(discrim = expr))]` with the second syntax,
/// where `expr` is a value that implements `IntoIterator<Item = Type2::Discrim>`
/// Note that `arch`, `comp` and `mut` must not be used if the second syntax is intended.
///
/// Similar to `name`, the expressions can read local and param states dirctly.
/// However, only the expressions are only resolved once before the first run of the system,
/// so mutating states has no effect on the system schedule.
///
/// # Example
/// ```no_run
/// # // TODO remove no_run here
/// use dynec::system;
///
/// #[dynec::global(initial = Title("hello world"))]
/// struct Title(&'static str);
///
/// #[derive(Debug, PartialEq, Eq, Hash)]
/// struct Foo;
///
/// dynec::archetype!(Player);
///
/// #[dynec::comp(of = Player)]
/// struct PositionX(f32);
/// #[dynec::comp(of = Player)]
/// struct PositionY(f32);
///
/// #[dynec::comp(of = Player)]
/// struct Direction(f32, f32);
///
/// #[derive(Debug, Clone, Copy)]
/// struct SkillId(usize);
/// impl dynec::comp::Discrim for SkillId {
///     fn from_usize(id: usize) -> Self { Self(id) }
///     fn to_usize(self) -> usize { self.0 }
/// }
///
/// #[dynec::comp(of = Player, isotope = SkillId, init = || SkillLevel(1))]
/// struct SkillLevel(u8);
///
/// #[system(
///     name = format!("simulate[counter = {}, skill_id = {:?}]", counter, skill_id),
///     before(Foo),
/// )]
/// fn simulate(
///     #[dynec(local = 0)] counter: &mut u16,
///     #[dynec(param)] skill_id: &SkillId,
///     #[dynec(global)] title: &mut Title,
///     x: system::Simple<Player, &mut PositionX>,
///     y: system::Simple<Player, &mut PositionY>,
///     dir: system::Simple<Player, &Direction>,
///     #[dynec(isotope(discrim = [*skill_id]))] skill: system::Isotope<Player, &SkillLevel>,
/// ) {
///     *counter += 1;
///
///     if *counter == 1 {
///         title.0 = "changed";
///     }
/// }
///
/// let system = simulate.build(SkillId(3));
/// assert_eq!(
///     system::System::get_spec(&system).debug_name.as_str(),
///     "simulate[counter = 0, skill_id = SkillId(3)]"
/// );
///
/// {
///     // We can also call the function directly in unit tests.
///
///     let mut counter = 0;
///     let mut title = Title("original");
///
///     let mut world = dynec::system_test! {
///         simulate.build(SkillId(2));
///         _: Player = (
///             PositionX(0.0),
///             PositionY(0.0),
///             Direction(0.5, 0.5),
///         );
///         _: Player = (
///             PositionX(0.5),
///             PositionY(0.5),
///             Direction(0.5, 0.5),
///         );
///     };
///
///     simulate(
///         &mut counter,
///         &SkillId(2),
///         &mut title,
///         todo!("component access logic"),
///         todo!("component access logic"),
///         todo!("component access logic"),
///         todo!("component access logic"),
///     );
///
///     assert_eq!(counter, 1);
///     assert_eq!(title.0, "changed");
/// }
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

        let system = simulate.build(2i64);
        {
            use crate::system::System;
            assert_eq!(
                system.get_spec().debug_name.as_str(),
                "dynec::macros::system_tests::simulate"
            );
        }
    }
}

/// Derives a [`crate::entity::Referrer`] implementation for the type.
///
/// The generated implementation does not visit any fields by default.
/// Add the `#[entity]` attribute to fields that implement `[crate::entity::Referrer]`,
/// then the generated implementation will delegate to these fields.
///
/// This derive macro is automatically called in [`comp`] and [`global`].
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

/// Convenience macro that constructs a new world for testing a small number of systems.
///
/// See [`system`] for example usage.
#[macro_export]
macro_rules! system_test {
    (
        $($systems:expr),* ;
        $(
            $var:tt : $arch:ty = ($($components:tt)*);
        )*
    ) => {{
        let mut builder = $crate::world::Builder::default();
        $(
            builder.schedule(Box::new($systems));
        )*

        #[allow(unused_mut)]
        let mut world = builder.build();

        $(
            let $var = world.create::<$arch>(
                $crate::comps![ $arch => $($components)*]
            );
        )*

        world
    }}
}
