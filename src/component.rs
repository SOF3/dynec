//! A component is a small data structure that can be attached to an entity.
//!
//! dynec is a statically-archetyped ECS framework.
//! Components can only be attached to entities of an archetype `A`
//! where the component type implements `Simple<A>` or `Isotope<A>`.
//!
//! # Simple vs Isotope components
//! A component type must implement either [`Simple`] or [`Isotope`] to be useful.
//!
//! Component types that implement [`Simple`] are called simple components.
//! There can only be one instance of each simple component type for each entity.
//! This means that each entity is effectively a typemap of its simple components.
//!
//! Component types that implement [`Isotope`] are called isotope components.
//! For an isotope component type `C`,
//! there can be multiple instances of `C` stored on the same entity,
//! indexed by [its discriminant type](Isotope::Discrim) [`Discrim`].
//! You can consider isotope components to be basically simple components with the type `Vec<C>`,
//! except with [more efficient storage](#storage).
//! Note that dynec instantiates a new storage for each discriminant,
//! so there should be a reasonably small number of distinct discriminants.
//!
//! # Registration
//! Archetypes and components are registered to the world
//! when a system that uses this archetype-component pair is scheduled.
//!
//! # Storage
//! For simple components, components of the same component type and archetype are stored together.
//!
//! For isotope components, components of the same component type, same archetype *and same
//! discriminant* are stored together.
//! This means each (component type + discriminant) combination is considered as a different
//! component type.
//!
//! For each storage, the components are either stored in a `Vec<MaybeUninit<C>>` or a `BTreeMap<C>`.
//!
//! If a simple component is optional and the storage is a `Vec<MaybeUninit<C>>`,
//! its presence in each entity is stored in a bitvec.
//! Isotope components are either lazy-initialized or optional,
//! so they always use a bitvec to store presence.
//!
//! # Instantiation
//! When an entity is created, its simple components are auto-instantiated based on the [`SimpleInitStrategy`]
//! specified in [`Simple::INIT_STRATEGY`] if it is absent in the creation args.
//!
//! Isotope components are never instantiated on entity creation.

use std::marker::PhantomData;

use crate::{entity, util, Archetype};

/// A simple component has only one instance per entity.
///
/// See the [module-level documentation](index.html) for more information.
pub trait Simple<A: Archetype>: Sized + 'static {
    /// The presence constraint of this component.
    const PRESENCE: SimplePresence;

    /// The initialization strategy for this component.
    const INIT_STRATEGY: SimpleInitStrategy<A, Self>;

    /// Override this to `true` if the component is a finalizer.
    ///
    /// Finalizer components must be [optional](SimplePresence::Optional).
    /// Entities are not removed until all finalizer components have been removed.
    const IS_FINALIZER: bool = false;
}

/// Describes whether a component must be present.
pub enum SimplePresence {
    /// The component may not be present in an entity.
    /// The component is always retrieved as an `Option` type.
    Optional,

    /// The component must be present in an entity.
    /// It can be mutated, but it cannot be removed from the entity.
    ///
    /// If it is not given in the entity creation args
    /// and its [`SimpleInitStrategy`] is not [`Auto`](SimpleInitStrategy::Auto),
    /// entity creation will panic.
    Required,
}

/// Describes how a component is auto-initialized.
pub enum SimpleInitStrategy<A: Archetype, C: Simple<A>> {
    /// The component is not auto-initialized.
    None,
    /// The component should be auto-initialized using the [`any::AutoIniter`]
    /// if it is not given in the creation args.
    Auto(any::AutoIniter<A, C>),
}

impl<A: Archetype, C: Simple<A>> SimpleInitStrategy<A, C> {
    /// Constructs an auto-initializing init strategy from a closure.
    pub fn auto(f: &'static impl any::AutoInitFn<A, C>) -> Self {
        Self::Auto(any::AutoIniter::new(f))
    }
}

/// An isotope component may have multiple instances per entity.
///
/// See the [module-level documentation](index.html) for more information.
pub trait Isotope<A: Archetype>: Sized + 'static {
    /// The discriminant type.
    type Discrim: Discrim;

    /// The initialzation strategy for this component.
    const INIT_STRATEGY: IsotopeInitStrategy<Self>;
}

/// Describes how an isotope component is auto-initialized.
pub enum IsotopeInitStrategy<T> {
    /// The component is not auto-initialized.
    /// The component is always retrieved as an `Option` type.
    None,
    /// The component should be auto-initialized using the given function
    /// if it is not already present when retrieved.
    ///
    /// For immutable access, if the value is not already present,
    /// the function is invoked each time the component is requested
    /// to pass the result to the system,
    /// but the result is not stored to avoid acquiring mutable access to the storage.
    /// Therefore, the function should be cheap, e.g. just creating a zero value.
    ///
    /// For mutable access, if the value is not already present,
    /// the function is invoked and the result is stored in the storage,
    /// then the system is given a mutable reference to the value in the storage.
    Default(fn() -> T),
}

/// A discriminant value that distinguishes different isotopes of the same component type.
///
/// For compact storage, the discriminant should have a one-to-one mapping to the `usize` type.
/// The `usize` needs not be a small number; it can be any valid `usize`
/// as long as it is one-to-one and consistent.
pub trait Discrim: Copy {
    /// Constructs a discriminant from the usize.
    ///
    /// Can panic if the usize is not supported.
    fn from_usize(usize: usize) -> Self;

    /// Converts the discriminant to a usize.
    fn to_usize(self) -> usize;
}

impl Discrim for usize {
    fn from_usize(usize: usize) -> Self { usize }

    fn to_usize(self) -> usize { self }
}

/// Marks that a component type is always present.
///
/// # Safety
/// This trait must only be implemented by components that
/// either implement [`Simple`] with [`Simple::PRESENCE`] set to [`SimplePresence::Required`]
/// or implement [`Isotope`] with [`Isotope::INIT_STRATEGY`] set to [`IsotopeInitStrategy::Default`].
///
/// Implementing this trait incorrectly currently only causes a panic
/// and does not result in UB, but it may cause UB in the future.
pub unsafe trait Must {}

/// A special type that implements [`Retrievable`] like simple components,
/// but exposes a map-like interface to access isotope components,
/// as if isotopes were implemented as `BTreeMap<C::Discrim, C>`.
pub struct IsotopeMap<A: Archetype, R: util::Ref> {
    // TODO
    _ph: PhantomData<(A, R)>,
}

impl<A: Archetype, R: util::Ref> IsotopeMap<A, R>
where
    R::Target: Isotope<A>,
{
    /// Retrieve the isotope of the specified discriminant.
    ///
    /// # Return values
    /// If the isotope is present in the storage,
    /// returns `Some` referencing the storage value.
    ///
    /// For [`IsotopeInitStrategy::Default`],
    /// if the isotope is not yet present in the storage,
    /// returns `Some` referencing a temporary value
    /// created from the default constructor.
    /// This value is dropped after the system is called.
    ///
    /// For [`IsotopeInitStrategy::None`],
    /// returns `None` if the isotope is not present in the entity.
    ///
    /// # Panics
    /// Panics if the discriminant is restricted in the system spec.
    pub fn try_get(
        &self,
        entity: &dyn entity::Ref<A>,
        discrim: <R::Target as Isotope<A>>::Discrim,
    ) -> Option<&<R::Target as Isotope<A>>::Discrim> {
        todo!()
    }
}

impl<'t, A: Archetype, C: Isotope<A>> IsotopeMap<A, &'t mut C> {
    /// Retrieves a mutable reference to the isotope of the specified discriminant.
    ///
    /// # Return values
    /// If the isotope is present in the storage,
    /// returns `Some` referencing the storage value.
    ///
    /// For [`IsotopeInitStrategy::Default`],
    /// if the isotope is not yet present in the storage,
    /// the storage is populated with a new call to the default constructor,
    /// then `Some` is returned referencing the storage value.
    ///
    /// For [`IsotopeInitStrategy::None`],
    /// returns `None` if the isotope is not present in the entity.
    pub fn try_get_mut(&mut self, discrim: C::Discrim) -> Option<&mut C> { todo!() }
}

/// A trait implemented for [`Simple`] references an [`IsotopeMap`].
/// This trait is only used for early constraint checking in types that accept both types,
/// and is not really useful by itself.
pub trait Retrievable<A: Archetype>: sealed::Sealed<A> {}

mod sealed {
    pub trait Sealed<A> {}
}

impl<'t, A: Archetype, C: Simple<A>> sealed::Sealed<A> for &'t C {}
impl<'t, A: Archetype, C: Simple<A>> Retrievable<A> for &'t C {}

impl<'t, A: Archetype, C: Simple<A>> sealed::Sealed<A> for &'t mut C {}
impl<'t, A: Archetype, C: Simple<A>> Retrievable<A> for &'t mut C {}

impl<'t, A: Archetype, C: Isotope<A>> sealed::Sealed<A> for IsotopeMap<A, &'t C> {}
impl<'t, A: Archetype, C: Isotope<A>> Retrievable<A> for IsotopeMap<A, &'t C> {}

impl<'t, A: Archetype, C: Isotope<A>> sealed::Sealed<A> for IsotopeMap<A, &'t mut C> {}
impl<'t, A: Archetype, C: Isotope<A>> Retrievable<A> for IsotopeMap<A, &'t mut C> {}

pub(crate) mod any;
pub use any::{AutoInitFn, AutoIniter, Map};

mod tuple;
pub use tuple::Tuple;
