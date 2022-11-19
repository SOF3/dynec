//! Component accessor APIs to be used from systems.

use std::marker::PhantomData;
use std::{any, fmt};

use crate::{comp, entity, Archetype};

/// Generalizes [`ReadSimple`] and [`ReadIsotope`] for a specific discriminant
/// (through [`ReadIsotope::split`]).
pub trait Read<A: Archetype, C: 'static> {
    /// Returns an immutable reference to the component for the specified entity,
    /// or `None` if the component is not present in the entity.
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C>;

    /// Returns an immutable reference to the component for the specified entity.
    ///
    /// # Panics
    /// This method panics if the entity is not fully initialized yet.
    /// This happens when an entity is newly created and the cycle hasn't joined yet.
    fn get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> &C
    where
        C: comp::Must<A>,
    {
        match self.try_get(entity) {
            Some(comp) => comp,
            None => panic!(
                "Component {}/{} implements comp::Must but is not present",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        }
    }

    /// Return value of [`iter`](Self::iter).
    type Iter<'t>: Iterator<Item = (entity::TempRef<'t, A>, &'t C)>
    where
        Self: 't;
    /// Iterates over all initialized components in this storage.
    fn iter(&self) -> Self::Iter<'_>;

    /// Returns an [`Accessor`] implementor that yields `&C` for each entity.
    fn access(&self) -> MustReadAccessor<A, C, &Self>
    where
        C: comp::Must<A>,
    {
        MustReadAccessor(self, PhantomData)
    }

    /// Returns an [`Accessor`] implementor that yields `Option<&C>` for each entity.
    fn try_access(&self) -> TryReadAccessor<A, C, &Self> { TryReadAccessor(self, PhantomData) }
}

/// Generalizes [`WriteSimple`] and [`WriteIsotope`] for a specific discriminant
/// (through [`WriteIsotope::split_mut`]).
pub trait Write<A: Archetype, C: 'static>: Read<A, C> {
    /// Returns a mutable reference to the component for the specified entity,
    /// or `None` if the component is not present in the entity.
    ///
    /// Note that this method returns `Option<&mut C>`, not `&mut Option<C>`.
    /// This means setting the Option itself to `Some`/`None` will not modify any stored value.
    /// Use [`set`](Self::set) to add/remove a component.
    fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C>;

    /// Returns a mutable reference to the component for the specified entity.
    ///
    /// This method is infallible, assuming [`comp::Must`] is only implemented
    /// for components with [`Required`](comp::SimplePresence::Required) presence.
    fn get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> &mut C
    where
        C: comp::Must<A>,
    {
        match self.try_get_mut(entity) {
            Some(comp) => comp,
            None => panic!(
                "Component {}/{} implements comp::Must but is not present",
                any::type_name::<A>(),
                any::type_name::<C>(),
            ),
        }
    }

    /// Overwrites the component for the specified entity.
    ///
    /// Passing `None` to this method removes the component from the entity.
    /// This leads to a panic for components with [`comp::SimplePresence::Required`] presence.
    fn set<E: entity::Ref<Archetype = A>>(&mut self, entity: E, value: Option<C>) -> Option<C>;

    /// Return value of [`iter_mut`](Self::iter_mut).
    type IterMut<'t>: Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)>
    where
        Self: 't;
    /// Iterates over mutable references to all initialized components in this storage.
    fn iter_mut(&mut self) -> Self::IterMut<'_>;

    /// Returns an [`Accessor`] implementor that yields `&C` for each entity.
    fn access_mut(&mut self) -> MustWriteAccessor<A, C, &mut Self>
    where
        C: comp::Must<A>,
    {
        MustWriteAccessor(self, PhantomData)
    }

    /// Returns an [`Accessor`] implementor that yields `Option<&C>` for each entity.
    fn try_access_mut(&mut self) -> TryWriteAccessor<A, C, &mut Self> {
        TryWriteAccessor(self, PhantomData)
    }
}

/// Provides access to a simple component in a specific archetype.
pub trait ReadSimple<A: Archetype, C: comp::Simple<A>>: Read<A, C> {}

/// Provides access to a simple component in a specific archetype.
pub trait WriteSimple<A: Archetype, C: comp::Simple<A>>: ReadSimple<A, C> + Write<A, C> {}

/// Provides access to an isotope component in a specific archetype.
///
/// `K` is the type used to index the discriminant.
/// For partial isotope access, `K` is usually `usize`.
/// For full isotope access, `K` is the discriminant type.
pub trait ReadIsotope<A: Archetype, C: comp::Isotope<A>, K = <C as comp::Isotope<A>>::Discrim>
where
    K: fmt::Debug + Copy + 'static,
{
    /// Retrieves the component for the given entity and discriminant.
    ///
    /// This method is infallible for correctly implemented `comp::Must`,
    /// which returns the auto-initialized value for missing components.
    fn get<E: entity::Ref<Archetype = A>>(&self, entity: E, discrim: K) -> &C
    where
        C: comp::Must<A>,
    {
        match self.try_get(entity, discrim) {
            Some(value) => value,
            None => panic!(
                "{}: comp::Must<{}> but has no default initializer",
                any::type_name::<C>(),
                any::type_name::<A>()
            ),
        }
    }

    /// Returns an immutable reference to the component for the specified entity and discriminant,
    /// or the default value for isotopes with a default initializer or `None`
    /// if the component is not present in the entity.
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E, discrim: K) -> Option<&C>;

    /// Return value of [`get_all`](Self::get_all).
    type GetAll<'t>: Iterator<Item = (<C as comp::Isotope<A>>::Discrim, &'t C)> + 't
    where
        Self: 't;
    /// Iterates over all isotopes of the component type for the given entity.
    ///
    /// The yielded discriminants are not in any guaranteed order.
    fn get_all<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Self::GetAll<'_>;

    /// Return value of [`iter`](Self::iter).
    type Iter<'t>: Iterator<Item = (entity::TempRef<'t, A>, &'t C)>
    where
        Self: 't;
    /// Iterates over all components of a specific discriminant.
    ///
    /// Note that the initializer is not called for lazy-initialized isotope components.
    /// To avoid confusing behavior, do not use this function if [`C: comp::Must<A>`](comp::Must).
    fn iter(&self, discrim: K) -> Self::Iter<'_>;

    /// Return value of [`split`](Self::split).
    type Split<'t>: Read<A, C> + 't
    where
        Self: 't;
    /// Splits the accessor into multiple [`Read`] implementors
    /// so that they can be used independently.
    fn split<const N: usize>(&self, keys: [K; N]) -> [Self::Split<'_>; N];
}

/// Provides access to an isotope component in a specific archetype.
pub trait WriteIsotope<A: Archetype, C: comp::Isotope<A>, K = <C as comp::Isotope<A>>::Discrim>:
    ReadIsotope<A, C, K>
where
    K: fmt::Debug + Copy + 'static,
{
    /// Returns a mutable reference to the component for the specified entity and discriminant,
    /// automatically initialized with the default initializer if present,
    /// or `None` if the component is unset and has no default initializer.
    ///
    /// Note that this method returns `Option<&mut C>`, not `&mut Option<C>`.
    /// This means setting the Option itself to `Some`/`None` will not modify any stored value.
    /// Use [`WriteIsotope::set`] to add/remove a component.
    fn try_get_mut<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        discrim: K,
    ) -> Option<&mut C>;

    /// Overwrites the component for the specified entity and discriminant.
    ///
    /// Passing `None` to this method removes the component from the entity.
    fn set<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        discrim: K,
        value: Option<C>,
    ) -> Option<C>;

    /// Return value of [`iter_mut`](Self::iter_mut).
    type IterMut<'t>: Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)>
    where
        Self: 't;
    /// Iterates over mutable references to all components of a specific discriminant.
    fn iter_mut(&mut self, discrim: K) -> Self::IterMut<'_>;

    /// Return value of [`split_mut`](Self::split_mut).
    type SplitMut<'t>: Write<A, C> + 't
    where
        Self: 't;
    /// Splits the accessor into multiple [`Write`] implementors
    /// so that they can be used in entity iteration independently.
    fn split_mut<const N: usize>(&mut self, keys: [K; N]) -> [Self::SplitMut<'_>; N];
}

/// An accessor that can be used in an entity iteration.
///
/// # Safety
/// Implementors must ensure that [`entity`](Self::entity) is deterministic and [one-to-one][injective].
///
/// Multiplexing implementors (such as tuples or composite accessors)
/// preserve this invariant automatically since they are just destructuring to independent storages.
/// Storage delegations preserve this invariant automatically
/// since [`Storage::get_mut`](crate::storage::Storage::get_mut)
/// has the same safety invariants
/// (see [`Storage` &sect; Safety](crate::storage::Storage#safety)).
///
/// [injective]: https://en.wikipedia.org/wiki/Injective_function
pub unsafe trait Accessor<A: Archetype> {
    /// Return value of [`entity`](Self::entity).
    type Entity<'t>: 't
    where
        Self: 't;
    /// Accesses this storage for a specific entity.
    ///
    /// # Safety
    /// The lifetime of the return value is arbitrarily defined by the caller.
    /// This effectively disables the borrow checker for return values.
    /// The caller must ensure that return values do not outlive `self`,
    /// and the function result is dropped before it is called again with the same `id`.
    unsafe fn entity<'this, 'e, 'ret>(
        this: &'this mut Self,
        id: entity::TempRef<'e, A>,
    ) -> Self::Entity<'ret>
    where
        Self: 'ret;
}

/// An accessor that can be used in chunked entity iteration.
///
/// # Safety
/// Implementors must ensure that [`chunk`](Self::chunk) is deterministic,
/// and non-overlapping entity chunks return non-overlapping values.
/// This is equivalent to (and should delegate to)
/// [`crate::storage::Chunked::get_chunk`]/[`crate::storage::Chunked::get_chunk_mut`].
///
/// Multiplexing implementors (such as tuples or composite accessors)
/// preserve this invariant automatically since they are just destructuring to independent storages.
/// Storage delegations preserve this invariant automatically
/// since [`crate::storage::Chunked::get_chunk_mut`] has the same safety invariants
/// (see [`Chunked` &sect; Safety](crate::storage::Chunked#safety)).
///
/// [injective]: https://en.wikipedia.org/wiki/Injective_function
pub unsafe trait Chunked<A: Archetype> {
    /// Return value of [`chunk`](Self::chunk).
    type Chunk<'t>: 't
    where
        Self: 't;
    /// Accesses this storage for a specific chunk of entities.
    ///
    /// # Safety
    /// The lifetime of the return value is arbitrarily defined by the caller.
    /// This effectively disables the borrow checker for return values.
    /// The caller must ensure that return values do not outlive `self`,
    /// and the function result is dropped before it is called again with an overlapping `chunk`.
    unsafe fn chunk<'this, 'e, 'ret>(
        this: &'this mut Self,
        chunk: entity::TempRefChunk<'e, A>,
    ) -> Self::Chunk<'ret>;
}

/// Return value of [`Read::try_access`].
pub struct TryReadAccessor<A, C, T>(T, PhantomData<(A, C)>);

unsafe impl<'t, A: Archetype, C: 'static, T: Read<A, C>> Accessor<A>
    for TryReadAccessor<A, C, &'t T>
{
    type Entity<'ret> = Option<&'ret C> where Self: 'ret;

    unsafe fn entity<'this, 'e, 'ret>(
        this: &'this mut Self,
        id: entity::TempRef<'e, A>,
    ) -> Self::Entity<'ret>
    where
        Self: 'ret,
    {
        Some(&*(this.0.try_get(id)? as *const C))
    }
}

/// Return value of [`Read::access`].
pub struct MustReadAccessor<A, C, T>(T, PhantomData<(A, C)>);

unsafe impl<'t, A: Archetype, C: comp::Must<A> + 'static, T: Read<A, C>> Accessor<A>
    for MustReadAccessor<A, C, &'t T>
{
    type Entity<'ret> = &'ret C where Self: 'ret;

    unsafe fn entity<'this, 'e, 'ret>(
        this: &'this mut Self,
        id: entity::TempRef<'e, A>,
    ) -> Self::Entity<'ret>
    where
        Self: 'ret,
    {
        &*(this.0.get(id) as *const C)
    }
}

/// Return value of [`Write::try_access_mut`].
pub struct TryWriteAccessor<A, C, T>(T, PhantomData<(A, C)>);

unsafe impl<'t, A: Archetype, C: 'static, T: Write<A, C>> Accessor<A>
    for TryWriteAccessor<A, C, &'t mut T>
{
    type Entity<'ret> = Option<&'ret mut C> where Self: 'ret;

    unsafe fn entity<'this, 'e, 'ret>(
        this: &'this mut Self,
        id: entity::TempRef<'e, A>,
    ) -> Self::Entity<'ret>
    where
        Self: 'ret,
    {
        Some(&mut *(this.0.try_get_mut(id)? as *mut C))
    }
}

/// Return value of [`Write::access_mut`].
pub struct MustWriteAccessor<A, C, T>(T, PhantomData<(A, C)>);

unsafe impl<'t, A: Archetype, C: comp::Must<A> + 'static, T: Write<A, C>> Accessor<A>
    for MustWriteAccessor<A, C, &'t mut T>
{
    type Entity<'ret> = &'ret C where Self: 'ret;

    unsafe fn entity<'this, 'e, 'ret>(
        this: &'this mut Self,
        id: entity::TempRef<'e, A>,
    ) -> Self::Entity<'ret>
    where
        Self: 'ret,
    {
        &mut *(this.0.get_mut(id) as *mut C)
    }
}

mod tuple_impl;
