/// Declares an [accessor set struct](crate::system::accessor::Set).
///
/// # Example
/// ```
/// dynec::accessors! {
///     /// This is an example accessor set struct.
///     /// We can document it and apply attributes on it.
///     #[allow(dead_code)]
///     pub Foo {
///         /// This documents the field.
///         pub(crate) bar,
///         qux,
///     }
/// }
/// ```
#[doc(inline)]
pub use dynec_codegen::accessors;

#[cfg(test)]
mod accessors_tests {}

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
/// Can be applied multiple times in the same attribute.
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
/// For isotope components, the initializer should return an iterator of `(C::Discrim, C)` tuples,
/// which is similar to the iterator from a HashMap when values of `C` are indexed by the
/// discriminant.
///
/// ## `storage`
/// Specify the [storage](crate::world::storage) type for the component.
/// The argument should be a path that specifies the target type.
/// If the all segments of the path does not have type parameters,
/// it is automatically filled with `<Arch::RawEntity, Self>`,
/// which is the format automatically compatible with all default storage types.
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
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec::Discrim)]
/// #[dynec(map = discrim::SortedVecMap)]
/// struct Id(usize);
///
/// #[comp(of = Foo, isotope = Id, init = Self::make/0)]
/// struct Corge(i32);
///
/// impl Corge {
///     fn make() -> [(Id, Self); 2] { [
///         (Id(3), Self(7)),
///         (Id(13), Self(17)),
///     ] }
/// }
/// ```
#[doc(inline)]
pub use dynec_codegen::comp;

#[cfg(test)]
mod comp_tests {}

/// Creates a map of components for a given archetype.
///
/// # Syntax
/// The macro starts with the archetype, followed by `=>`,
/// then a comma-delimited list of simple and isotope components.
///
/// ## Simple components
/// Simple components can be passed in the list directly.
///
/// If it is not known whether a component should be added to the list at compile time,
/// start with `@?`, followed by a value of type `Option<Simple>`, e.g.
/// `@?flag.then_with(|| value)`.
///
/// ## Isotope components
/// For each isotope component, start with a `@`,
/// followed by a tuple of type `(Discrim, Isotope)`,
/// e.g. `@(discrim, value)`.
///
/// Since there can be multiple isotope components for the same entity,
/// an iterator of isotope tuples is also allowed.
/// Start with `@?`, followed by a value that implements
/// <code>[IntoIterator]&lt;Item = (Discrim, Isotope)&gt;</code>.
/// <code>[HashMap](std::collections::HashMap)&lt;Discrim, Isotope&gt;</code> and
/// <code>[BTreeMap](std::collections::BTreeMap)&lt;Discrim, Isotope&gt;</code>
/// satisfy this requirement automatically.
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
/// struct Comp3 { value: i32 }
/// #[dynec::comp(of = Foo)]
/// struct Comp4 { value: i32 }
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec::Discrim)]
/// struct MyDiscrim(usize);
///
/// #[dynec::comp(of = Foo, isotope = MyDiscrim)]
/// struct Iso(&'static str);
///
/// #[dynec::comp(of = Foo, isotope = MyDiscrim)]
/// struct Carbon { neutrons: i32 };
///
/// let mut hashed = std::collections::HashMap::new();
/// hashed.insert(MyDiscrim(10), Carbon { neutrons: 4 });
/// hashed.insert(MyDiscrim(11), Carbon { neutrons: 5 });
/// hashed.insert(MyDiscrim(12), Carbon { neutrons: 6 });
///
/// let map = dynec::comps![Foo =>
///     Comp1,
///     Comp2(2),
///     ?Some(Comp3{ value: 3 }),
///     ?None::<Comp4>,
///     @(MyDiscrim(4), Iso("xxx")),
///     @?hashed,
/// ];
/// assert_eq!(map.len(), 7);
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
/// If a required global state has no initial value
/// and it is not set in the builder,
/// building the world would panic.
///
/// ```should_panic
/// #[dynec::global]
/// struct Qux(i32);
///
/// #[dynec::system]
/// fn test_system(#[dynec(global)] _qux: &Qux) {}
///
/// let mut builder = dynec::world::Builder::new(1);
/// builder.schedule(Box::new(test_system.build()));
/// builder.build();
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
/// Similar to `name`, the expressions can read local and param states directly.
/// However, only the expressions are only resolved once before the first run of the system,
/// so mutating states has no effect on the system schedule.
///
/// # Parameters
/// Each parameter of a system function has a special meaning:
///
/// ## Local states
/// Parameters with the attribute `#[dynec(local(initial = xxx))]` are "local states",
/// where `xxx` is an expression that evaluates to the initial value of the state.
///
/// Local states must take the type `&T` or `&mut T`,
/// where `T` is the actual stored state.
/// The mutated state persists for each instance of the system.
///
/// Use global states instead if the local state needs to be accessed from multiple systems.
///
/// Since entity references can be stored in local states,
/// the struct used to store local states also implements
/// [`entity::Referrer`](crate::entity::Referrer).
/// The corresponding `entity` and `not_entity` attributes can be inside the `local()` instead.
///
/// Unlike global states, local states do not need to specify thread safety.
/// Thread safety of local states is checked at compile time
/// when the system is passed to the scheduler.
///
/// ### Syntax reference
/// ```
/// # /*
/// #[dynec(local(
///     // Required, the initial value of the local state.
///     initial = $expr,
///     // Optional, equivalent to #[entity] in #[derive(EntityRef)].
///     entity,
///     // Optional, equivalent to #[not_entity] in #[derive(EntityRef)].
///     not_entity,
/// ))]
/// # */
/// ```
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
/// Similar to local states, param states can also use `entity` and `not_entity`.
///
/// ### Syntax reference
/// ```
/// # /*
/// #[dynec(param(
///     // Optional, equivalent to #[entity] in #[derive(EntityRef)].
///     entity,
///     // Optional, equivalent to #[not_entity] in #[derive(EntityRef)].
///     not_entity,
/// ))]
/// # */
/// ```
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
/// ### Syntax reference
/// ```
/// # /*
/// #[dynec(global(
///     // Optional, indicates that the global state is not thread-safe.
///     // Forgetting to mark `thread_local` will result in compile error.
///     thread_local,
///     // Optional, acknowledges that the entities of the specified archetypes
///     // contained in the global state may be uninitialized.
///     maybe_uninit($ty, $ty, ...),
/// ))]
/// # */
/// ```
///
/// ## Simple components
/// Parameters in the form `impl ReadSimple<A, C>` or `impl WriteSimple<A, C>`,
/// request access to a [simple component](crate::comp::Simple) of type `C`
/// from entities of the [archetype](crate::Archetype) `A`,
/// exposed through a type that implements [`system::ReadSimple`](crate::system::ReadSimple)
/// or [`system::WriteSimple`](crate::system::WriteSimple).
/// The latter provides mutable and exclusive access to the component storages.
///
/// ### Using other bounds
/// Other trait bounds for the parameter are also allowed,
/// but the macro would not be able to infer type parameters and mutability.
/// In such cases, they must be indicated explicitly in the attribute.
/// See the syntax reference below for details.
///
/// ### Uninitialized entity references.
/// Entity creation ordering is automatically enforced if `C` contains entity references,
/// Use the `maybe_uninit` attribute to remove this ordering.
///
/// See [`EntityCreationPartition`](crate::system::partition::EntityCreationPartition#component-accessors)
/// for more information.
///
/// ### Syntax reference
/// ```
/// # /*
/// #[dynec(simple(
///     // Optional, specifies the archetype and component explicitly.
///     // Only required when the parameter type is not `impl ReadSimple`/`impl WriteSimple`.
///     arch = $ty, comp = $ty,
///     // Optional, indicates that the component access is exclusive explicitly.
///     // Only required when the parameter type is not `impl WriteSimple`.
///     mut,
///     // Optional, acknowledges that the entities of the specified archetypes
///     // contained in the simple components may be uninitialized.
///     maybe_uninit($ty, $ty, ...),
/// ))]
/// # */
/// ```
///
/// ## Isotope components
/// Parameters in the form `impl ReadIsotope<A, C>` or `impl WriteIsotope<A, C>`,
/// request access to an [isotope component](crate::comp::Isotope) of type `C`
/// from entities of the [archetype](crate::Archetype) `A`,
/// exposed through a type that implements [`system::ReadIsotope`](crate::system::ReadIsotope)
/// or [`system::WriteIsotope`](crate::system::WriteIsotope).
/// The latter provides mutable and exclusive access to the component storages.
///
/// ### Partial isotope access
/// By default, all discriminants of the isotope component are requested,
/// such that writes are exclusive with all systems that read any part of the discriminants.
/// The accessor can be made partial instead:
///
/// ```
/// # /*
/// #[dynec(isotope(discrim = discrim_set))] param_name: impl ReadIsotope<A, C, K>,
/// # */
/// ```
///
/// The expression `discrim_set` implements
/// <code>[discrim::Set](crate::comp::discrim::Set)<C::[Discrim](crate::comp::Discrim)></code>,
/// which is the set of discriminants that this system uses.
/// The expression can reference local and param states directly.
/// However, since it is only evaluated once before the first run of the system,
/// subsequent writes to the states have no effect on the resolved discriminant set.
///
/// `K` is the type of the [key](crate::comp::discrim::Set::Key) to index the discriminant set.
///
/// See the documentation of [`discrim::Set`](crate::comp::discrim::Set) for more information.
///
/// ### Using other bounds
/// Other trait bounds for the parameter are also allowed,
/// but the macro would not be able to infer type parameters and mutability.
/// In such cases, they must be indicated explicitly in the attribute.
/// See the syntax reference below for details.
///
/// ### Uninitialized entity references.
/// Entity creation ordering is automatically enforced if `C` contains entity references,
/// Use the `maybe_uninit` attribute to remove this ordering.
///
/// See [`EntityCreationPartition`](crate::system::partition::EntityCreationPartition#component-accessors)
/// for more information.
///
/// ### Syntax reference
/// ```
/// # /*
/// #[dynec(isotope(
///     // Optional, indicates that this accessor only uses the given subset of discriminants.
///     discrim = $expr,
///     // Optional, must be the same as the `Key` associated type of the `discrim` expression.
///     // Only required when the parameter type is not `impl ReadIsotope`/`impl WriteIsotope`.
///     discrim_key = $ty,
///     // Optional, specifies the archetype and component explicitly.
///     // Only required when the parameter type is not `impl ReadIsotope`/`impl WriteIsotope`.
///     arch = $ty, comp = $ty,
///     // Optional, indicates that the component access is exclusive explicitly.
///     // Only required when the parameter type is not `impl WriteSimple`.
///     mut,
///     // Optional, acknowledges that the entities of the specified archetypes
///     // contained in the simple components may be uninitialized.
///     maybe_uninit($ty, $ty, ...),
/// ))]
/// # */
/// ```
///
/// ## Entity creation
/// Parameters that require an implementation of [`EntityCreator`](crate::system::EntityCreator)
/// can be used to create entities.
/// The archetype of created entities is specified in the type bounds.
/// Note that entity creation is asynchronous to ensure synchronization,
/// i.e. components of the created entity are deferred until the current cycle completes.
///
/// Systems that create entities of an archetype `A` should be scheduled to execute
/// after all systems that may read entity references of archetype `A`
/// (through strong or weak references stored in
/// local states, global states, simple components or isotope components).
/// See [`EntityCreationPartition`](crate::system::partition::EntityCreationPartition#entity-creators)
/// for more information.
///
/// If it can be ensured that the new uninitialized entities cannot be leaked to other systems,
/// e.g. if the created entity ID is not stored into any states,
/// the attribute `#[dynec(entity_creator(no_partition))]`
/// can be applied on the entity-creating parameter
/// to avoid registering the automatic dependency to run after `EntityCreationPartition<A>`.
///
/// ### Syntax reference
/// ```
/// # /*
/// /// This attribute is not required unless `EntityCreator` is aliased.
/// #[dynec(entity_creator(
///     // Optional, specifies the archetype if `EntityCreator` is aliased.
///     arch = $ty,
///     // Optional, allows the derived system to execute before
///     // the EntityCreationPartition of this archetype.
///     no_partition,
/// ))]
/// # */
/// ```
///
/// ## Entity deletion
/// Parameters that require an implementation of [`EntityDeleter`](crate::system::EntityDeleter)
/// can be used to delete entities.
/// The archetype of deleted entities is specified in the type bounds.
/// Note that `EntityDeleter` can only be used to mark entities as "deleting";
/// the entity is only deleted after
/// all [finalizer](crate::comp::Simple::IS_FINALIZER) components are unset.
///
/// It is advisable to execute finalizer-removing systems
/// after systems that mark entities for deletion finish executing.
/// This allows deletion to happen in the same cycle,
/// thus slightly improving entity deletion performance
/// (but this is not supposed to be critical anyway).
/// Nevertheless, unlike entity creation, entity deletion does not have an automatic partition.
///
/// ### Syntax reference
/// ```
/// # /*
/// /// This attribute is not required unless `EntityDeleter` is aliased.
/// #[dynec(entity_deleter(
///     // Optional, specifies the archetype if `EntityDeleter` is aliased.
///     arch = $ty,
/// ))]
/// # */
/// ```
///
/// # Example
/// ```
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
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec::Discrim)]
/// struct SkillType(usize);
///
/// #[dynec::comp(of = Player, isotope = SkillType, init = || [(SkillType(0), SkillLevel(1))])]
/// struct SkillLevel(u8);
///
/// #[system(
///     name = format!("simulate[counter = {}, skill_id = {:?}]", counter, skill_id),
///     before(Foo),
/// )]
/// fn simulate(
///     #[dynec(local(initial = 0))] counter: &mut u16,
///     #[dynec(param)] &skill_id: &SkillType,
///     #[dynec(global)] title: &mut Title,
///     x: impl system::WriteSimple<Player, PositionX>,
///     y: impl system::WriteSimple<Player, PositionY>,
///     dir: impl system::ReadSimple<Player, Direction>,
///     #[dynec(isotope(discrim = [skill_id]))] skill: impl system::ReadIsotope<
///         Player,
///         SkillLevel,
///         usize,
///     >,
/// ) {
///     *counter += 1;
///
///     if *counter == 1 {
///         title.0 = "changed";
///     }
/// }
///
/// let system = simulate.build(SkillType(3));
/// assert_eq!(
///     system::Descriptor::get_spec(&system).debug_name.as_str(),
///     "simulate[counter = 0, skill_id = SkillType(3)]"
/// );
///
/// {
///     // We can also call the function directly in unit tests.
///
///     let mut counter = 0;
///     let mut title = Title("original");
///
///     let mut world = dynec::system_test! {
///         simulate.build(SkillType(2));
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
///     simulate::call(
///         &mut counter,
///         &SkillType(2),
///         &mut title,
///         world.components.write_simple_storage(),
///         world.components.write_simple_storage(),
///         world.components.read_simple_storage(),
///         world.components.read_partial_isotope_storage(&[SkillType(3)]),
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
            #[dynec(local(initial = 0))] counter_one: &mut u16,
            #[dynec(param)] counter_two: &mut i64,
        ) {
            *counter_one += 1u16;
            *counter_two += 3i64;
        }

        let system = simulate.build(2i64);
        {
            use crate::system::Descriptor;
            assert_eq!(
                system.get_spec().debug_name.as_str(),
                "dynec::macros::system_tests::simulate"
            );
        }
    }
}

/// Derives a [`Discrim`](crate::comp::Discrim) implementation for the type.
///
/// This derive macro is only applicable to
/// single-field structs (both tuple structs and named structs)
/// and enums with only unit fields.
///
/// For structs, the only field must be an integer type
/// (one that is convertible from and to `usize` through [`xias::SmallInt`]).
/// Note that dynec mostly uses `usize` to identify isotopes,
/// so using `u8` instead of `usize` as the backing type does not provide notable benefit;
/// custom discriminant types are only available for ergonomic reasons.
///
/// # Customizing the discriminant map
/// Implementations for structs use
/// [`discrim::BoundedVecMap`](crate::comp::discrim::BoundedVecMap) by default,
/// which is optimized for small discriminants.
/// This can be customized by adding `#[dynec(map = path::to::another::Impl)]` on the struct.
/// [`dynec::comp::discrim`](crate::comp::discrim)
/// is automatically imported for the map reference,
/// so users only need to specify e.g. `#[dynec(map = discrim::LinearVecMap)]`.
///
/// Since maps are generic over `T`,
/// the passed type actually can depend on the type parameter `T`,
/// e.g. `#[dynec(map = discrim::ArrayMap<Self, T, 16>)]`.
/// Inputs without trailing type parameters are appended with `<Self, T>` automatically,
/// where `Self` is the derived type.
///
/// Enums do not require customization because they always use
/// [`ArrayMap`](crate::comp::discrim::ArrayMap).
///
/// # Example
/// ```
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec::Discrim)]
/// struct Tuple(u16);
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec::Discrim)]
/// #[dynec(map = discrim::SortedVecMap)]
/// struct Named {
///     field: u32,
/// }
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec::Discrim)]
/// #[dynec(map = discrim::ArrayMap<Self, T, 16>)]
/// struct UsesArray {
///     field: u8,
/// }
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec::Discrim)]
/// enum Enum {
///     Foo,
///     Bar,
/// }
/// ```
#[doc(inline)]
pub use dynec_codegen::Discrim;

#[cfg(test)]
mod discrim_tests {}

/// Derives a [`Referrer`](crate::entity::Referrer) implementation for the type.
///
/// The generated implementation does not visit any fields by default.
/// Add the `#[entity]` attribute to fields that implement `[crate::entity::Referrer]`,
/// then the generated implementation will delegate to these fields.
///
/// This derive macro is automatically called in [`comp`] and [`global`].
/// It should only be called explicitly if the type is not a component or global,
/// e.g. if it is a type included in a component field.
///
/// # Example
/// ```
/// dynec::archetype!(Foo);
///
/// #[derive(dynec::EntityRef)]
/// struct Bar {
///     #[entity]
///     entity: dynec::Entity<Foo>,
/// }
/// ```
///
/// A compile error would be triggered if a field is an entity reference but is not `#[entity]`:
///
/// ```compile_fail
/// dynec::archetype!(Foo);
///
/// #[derive(dynec::EntityRef)]
/// struct Bar {
///     entity: dynec::Entity<Foo>,
/// }
/// ```
///
/// The above code will fail to compile with an error that contains
/// `this_field_references_an_entity_so_it_should_have_the_entity_attribute`.
///
/// In the case where a field references a type parameter,
/// dynec cannot check whether it correctly does not implement `Referrer`.
/// In that case, apply the `#[not_entity]` attribute to assert its safety:
///
/// ```
/// # dynec::archetype!(Foo);
/// #
/// #[derive(dynec::EntityRef)]
/// struct Bar<T: 'static> {
///     #[not_entity]
///     value: T,
/// }
/// ```
///
/// It is the user's responsibility not to set `T` as a `Referrer` implementation.
///
/// Note that this compile error is best-effort and not comprehensive &mdash;
/// if the actual entity reference is hidden behind a complex type
/// that does not implement [`Referrer`](crate::entity::Referrer),
/// e.g. as an element in a tuple, this error will not happen,
/// which would lead to a runtime panic instead during ref counting.
#[doc(inline)]
pub use dynec_codegen::EntityRef;

#[cfg(test)]
mod entity_ref_tests {}

// The rest are macros for testing.

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
        let mut builder = $crate::world::Builder::new(0);
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

/// Asserts that a type can be used as a partition.
///
/// # Example
/// ```
/// use dynec::system::partition::EntityCreationPartition;
/// dynec::assert_partition!(EntityCreationPartition);
/// ```
#[macro_export]
macro_rules! assert_partition {
    (@expr $value:expr) => {
        const _: fn() = || {
            let _ = $crate::system::partition::Wrapper(Box::new($value));
        };
    };

    ($ty:ty) => {
        const _: fn($ty) = |value| {
            let _ = $crate::system::partition::Wrapper(Box::new(value));
        };
    };
}
