//! Component accessor APIs to be used from systems.

use std::marker::PhantomData;
use std::{any, fmt, ops};

use crate::{comp, entity, storage, Archetype};

/// Generalizes [`ReadSimple`] and specific-discriminant [`ReadIsotope`] (through [`with`]).
pub trait Read<A: Archetype, C: 'static> {
    /// Returns an immutable reference to the component for the specified entity,
    /// or `None` if the component is not present in the entity.
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C>;

    /// Returns a slice of contiguously located components as a slice.
    ///
    /// Since the internal representation does not use `[Option<C>]`,
    /// it is not meaningful to use this function on components that may be missing.
    /// Hence, this function only works on simple components that must exist in the chunk.
    fn get_chunk(&self, chunk: entity::TempRefChunk<'_, A>) -> &[C]
    where
        Self: comp::Must<A>,
        C: comp::Simple<A>,
        <C as comp::Simple<A>>::Storage: storage::Chunked;

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
}

/// Generalizes [`WriteSimple`] and specific-discriminant [`WriteIsotope`] (through [`with_mut`]).
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
}

/// Provides access to a simple component in a specific archetype.
pub trait ReadSimple<A: Archetype, C: comp::Simple<A>>: Read<A, C> {}

/// Provides access to a simple component in a specific archetype.
pub trait WriteSimple<A: Archetype, C: comp::Simple<A>>: ReadSimple<A, C> + Write<A, C> {}

/// Provides access to an isotope component in a specific archetype.
///
/// `K` is the type used to index the discriminant.
/// For partial
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
}

/// Create a single [`Read`] accessor from an isotope accessor for a fixed discriminant.
pub fn with<A, C, K, T>(accessor: &T, discrim: K) -> impl Read<A, C> + '_
where
    A: Archetype,
    C: comp::Isotope<A>,
    K: fmt::Debug + Copy + 'static,
    T: ReadIsotope<A, C, K>,
{
    With { accessor, discrim, _ph: PhantomData }
}

/// Create a single [`Write`] accessor from an isotope accessor for a fixed discriminant.
pub fn with_mut<A, C, K, T>(accessor: &mut T, discrim: K) -> impl Write<A, C> + '_
where
    A: Archetype,
    C: comp::Isotope<A>,
    K: fmt::Debug + Copy + 'static,
    T: WriteIsotope<A, C, K>,
{
    With { accessor, discrim, _ph: PhantomData }
}

struct With<A, C, K, R: ops::Deref> {
    accessor: R,
    discrim:  K,
    _ph:      PhantomData<(A, C)>,
}

impl<A, C, K, R: ops::Deref> Read<A, C> for With<A, C, K, R>
where
    A: Archetype,
    C: comp::Isotope<A>,
    K: fmt::Debug + Copy + 'static,
    <R as ops::Deref>::Target: ReadIsotope<A, C, K>,
{
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C> {
        self.accessor.try_get(entity, self.discrim)
    }

    fn get_chunk(&self, _chunk: entity::TempRefChunk<'_, A>) -> &[C]
    where
        Self: comp::Must<A>,
    {
        unreachable!("Isotope components should not implement comp::Must")
    }

    type Iter<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t C)>
    where
        Self: 't;
    fn iter(&self) -> Self::Iter<'_> { self.accessor.iter(self.discrim) }
}

impl<A, C, K, R: ops::DerefMut> Write<A, C> for With<A, C, K, R>
where
    A: Archetype,
    C: comp::Isotope<A>,
    K: fmt::Debug + Copy + 'static,
    <R as ops::Deref>::Target: WriteIsotope<A, C, K>,
{
    fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C> {
        self.accessor.try_get_mut(entity, self.discrim)
    }

    fn set<E: entity::Ref<Archetype = A>>(&mut self, entity: E, value: Option<C>) -> Option<C> {
        self.accessor.set(entity, self.discrim, value)
    }

    type IterMut<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)>
    where
        Self: 't;
    fn iter_mut(&mut self) -> Self::IterMut<'_> { self.accessor.iter_mut(self.discrim) }
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
/// (see [`Storage` &sect; Safety](storage::Storage#safety)).
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
    ) -> Self::Entity<'ret>;
}

/// An accessor that can be used in chunked entity iteration.
///
/// # Safety
/// Implementors must ensure that [`chunk`](Self::chunk) is deterministic,
/// and non-overlapping entity chunks return non-overlapping values.
/// This is equivalent to (and should delegate to)
/// [`storage::Chunked::get_chunk`]/[`storage::Chunked::get_chunk_mut`].
///
/// Multiplexing implementors (such as tuples or composite accessors)
/// preserve this invariant automatically since they are just destructuring to independent storages.
/// Storage delegations preserve this invariant automatically
/// since [`storage::Chunked::get_chunk_mut`] has the same safety invariants
/// (see [`Chunked` &sect; Safety](storage::Chunked#safety)).
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

mod tuple_impl;
