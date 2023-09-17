use std::marker::PhantomData;
use std::{any, fmt};

use super::accessor;
use crate::entity::ealloc;
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

    /// Returns an [`Accessor`](accessor::Accessor) implementor that yields `&C` for each entity.
    fn access(&self) -> accessor::MustRead<A, C, &Self>
    where
        C: comp::Must<A>,
    {
        accessor::MustRead(self, PhantomData)
    }

    /// Returns an [`Accessor`](accessor::Accessor) implementor that yields `Option<&C>` for each entity.
    fn try_access(&self) -> accessor::TryRead<A, C, &Self> { accessor::TryRead(self, PhantomData) }

    /// Return value of [`duplicate_immut`](Self::duplicate_immut).
    type DuplicateImmut<'t>: Read<A, C> + 't
    where
        Self: 't;
    /// Duplicates the current reader,
    /// producing two new values that can only access the storage immutably.
    fn duplicate_immut(&self) -> (Self::DuplicateImmut<'_>, Self::DuplicateImmut<'_>);

    /// Return value of [`par_iter`](Self::par_iter).
    type ParIter<'t>: rayon::iter::ParallelIterator<Item = (entity::TempRef<'t, A>, &'t C)>
    where
        Self: 't,
        C: comp::Must<A>;
    /// Iterates over chunks of entities in parallel.
    ///
    /// This returns a [rayon `ParallelIterator`](rayon::iter::ParallelIterator)
    /// that processes different chunks of entities
    fn par_iter<'t>(&'t self, snapshot: &'t ealloc::Snapshot<A::RawEntity>) -> Self::ParIter<'t>
    where
        C: comp::Must<A>;
}

/// Extends [`Read`] with chunk reading ability
/// for storages that support chunked access.
pub trait ReadChunk<A: Archetype, C: 'static> {
    /// Returns the chunk of components as a slice.
    ///
    /// # Panics
    /// This method panics if any component in the chunk is missing.
    /// In general, users should not get an [`entity::TempRefChunk`]
    /// that includes an uninitialized entity,
    /// so panic is basically impossible if [`comp::Must`] was implemented correctly.
    fn get_chunk(&self, chunk: entity::TempRefChunk<'_, A>) -> &'_ [C];

    /// Return value of [`par_iter_chunk`](Self::par_iter_chunk).
    type ParIterChunk<'t>: rayon::iter::ParallelIterator<
        Item = (entity::TempRefChunk<'t, A>, &'t [C]),
    >
    where
        Self: 't;
    /// Iterates over chunks of entities in parallel.
    ///
    /// This returns a [rayon `ParallelIterator`](rayon::iter::ParallelIterator)
    /// that processes different chunks of entities
    fn par_iter_chunk<'t>(
        &'t self,
        snapshot: &'t ealloc::Snapshot<A::RawEntity>,
    ) -> Self::ParIterChunk<'t>;
}

/// Generalizes [`WriteSimple`], [`WriteIsotope`] and their split storages.
///
/// Only supports mutable access to an existing component,
/// but does not support adding or removing components
/// since only the storage values but not the storage structure can be borrowed mutably.
pub trait Mut<A: Archetype, C: 'static> {
    /// Returns a mutable reference to the component for the specified entity,
    /// or `None` if the component is not present in the entity.
    ///
    /// Note that this method returns `Option<&mut C>`, not `&mut Option<C>`.
    /// This means setting the Option itself to `Some`/`None` will not modify any stored value.
    /// Use [`Write::set`] to add/remove a component.
    fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C>;

    /// Return value of [`iter_mut`](Self::iter_mut).
    type IterMut<'t>: Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)>
    where
        Self: 't;
    /// Iterates over mutable references to all initialized components in this storage.
    fn iter_mut(&mut self) -> Self::IterMut<'_>;

    /// Return value of [`split_entities_at`](Self::split_entities_at).
    type SplitEntitiesAt<'u>: Mut<A, C> + 'u
    where
        Self: 'u;
    /// Partitions the accessor into two disjoint halves of entities.
    ///
    /// This method is not required for [`Read`]
    /// because shared references can be reused directly.
    fn split_entities_at<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
    ) -> (Self::SplitEntitiesAt<'_>, Self::SplitEntitiesAt<'_>);
}

/// Generalizes [`WriteSimple`] and [`WriteIsotope`] for a specific discriminant
/// (through [`WriteIsotope::split_isotopes`]).
pub trait Write<A: Archetype, C: 'static>: Read<A, C> + Mut<A, C> {
    /// Returns a mutable reference to the component for the specified entity.
    ///
    /// This method is infallible, assuming [`comp::Must`] is only implemented
    /// for components with [`Required`](comp::Presence::Required) presence.
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
    /// This leads to a panic for components with [`comp::Presence::Required`] presence.
    fn set<E: entity::Ref<Archetype = A>>(&mut self, entity: E, value: Option<C>) -> Option<C>;

    /// Returns an [`Accessor`](accessor::Accessor) implementor that yields `&C` for each entity.
    fn access_mut(&mut self) -> accessor::MustWrite<A, C, &mut Self>
    where
        C: comp::Must<A>,
    {
        accessor::MustWrite(self, PhantomData)
    }

    /// Returns an [`Accessor`](accessor::Accessor) implementor that yields `Option<&C>` for each entity.
    fn try_access_mut(&mut self) -> accessor::TryWrite<A, C, &mut Self> {
        accessor::TryWrite(self, PhantomData)
    }

    /// Return value of [`par_iter_mut`](Self::par_iter_mut).
    type ParIterMut<'t>: rayon::iter::ParallelIterator<Item = (entity::TempRef<'t, A>, &'t mut C)>
    where
        Self: 't,
        C: comp::Must<A>;
    /// Iterates over chunks of entities in parallel.
    ///
    /// This returns a [rayon `ParallelIterator`](rayon::iter::ParallelIterator)
    /// that processes different chunks of entities
    fn par_iter_mut<'t>(
        &'t mut self,
        snapshot: &'t ealloc::Snapshot<A::RawEntity>,
    ) -> Self::ParIterMut<'t>
    where
        C: comp::Must<A>;
}

/// Extends [`Write`] with chunk writing ability
/// for storages that support chunked access.
pub trait WriteChunk<A: Archetype, C: 'static> {
    /// Returns the chunk of components as a mutable slice.
    /// Typically called from an accessor.
    ///
    /// # Panics
    /// This method panics if any component in the chunk is missing.
    /// In general, users should not get an [`entity::TempRefChunk`]
    /// that includes an uninitialized entity,
    /// so panic is basically impossible if [`comp::Must`] was implemented correctly.
    fn get_chunk_mut(&mut self, chunk: entity::TempRefChunk<'_, A>) -> &'_ mut [C]
    where
        C: comp::Must<A>;
}

/// Provides access to a simple component in a specific archetype.
pub trait ReadSimple<A: Archetype, C: comp::Simple<A>>: Read<A, C> {
    /// Returns a [`Chunked`](accessor::Chunked) accessor that can be used in
    /// [`EntityIterator`](super::EntityIterator)
    /// to provide chunked iteration to an entity.
    fn access_chunk(&self) -> accessor::MustReadChunkSimple<'_, A, C>;
}

/// Provides access to a simple component in a specific archetype.
pub trait WriteSimple<A: Archetype, C: comp::Simple<A>>: ReadSimple<A, C> + Write<A, C> {
    /// Returns a [`Chunked`](accessor::Chunked) accessor that can be used in
    /// [`EntityIterator`](super::EntityIterator)
    /// to provide chunked iteration to an entity.
    fn access_chunk_mut(&mut self) -> accessor::MustWriteChunkSimple<'_, A, C>;
}

/// Provides access to an isotope component in a specific archetype.
///
/// `K` is the type used to index the discriminant.
/// For partial isotope access, `K` is usually `usize`.
/// For full isotope access, `K` is the discriminant type.
///
/// Partial isotope access also implements [`ReadIsotopeRef`],
/// which allows using the accessor immutably
/// so that it can be dispatched to multiple workers.
///
/// For full accessors, getters require a mutable receiver
/// to allow lazy initialization of new discriminants.
/// Consider [splitting](Self::split) accessors,
/// which return [`Read`] with a shared receiver.
/// If it can be asserted that no uninitialized discriminants will be encountered,
/// use with [`known_discrims`].
///
/// Since mutable receiver is only required for initializing new isotopes,
/// functions that only work on known existing isotops such as [`known_discrims`]
/// only require a shared receiver.
pub trait ReadIsotope<A: Archetype, C: comp::Isotope<A>, K = <C as comp::Isotope<A>>::Discrim>
where
    K: fmt::Debug + Copy + 'static,
{
    /// Retrieves the component for the given entity and discriminant.
    ///
    /// This method is infallible for correctly implemented `comp::Must`,
    /// which returns the auto-initialized value for missing components.
    fn get<E: entity::Ref<Archetype = A>>(&mut self, entity: E, discrim: K) -> &C
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
    fn try_get<E: entity::Ref<Archetype = A>>(&mut self, entity: E, discrim: K) -> Option<&C>;

    /// Return value of [`known_discrims`](Self::known_discrims).
    type KnownDiscrims<'t>: Iterator<Item = <C as comp::Isotope<A>>::Discrim> + 't
    where
        Self: 't;
    /// Iterates over all known discriminants of the component type.
    ///
    /// The yielded discriminants are not in any guaranteed order.
    fn known_discrims(&self) -> Self::KnownDiscrims<'_>;

    /// Return value of [`get_all`](Self::get_all).
    type GetAll<'t>: Iterator<Item = (C::Discrim, &'t C)> + 't
    where
        Self: 't;
    /// Iterates over all known isotopes for a specific entity.
    fn get_all<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Self::GetAll<'_>;

    /// Return value of [`iter`](Self::iter).
    type Iter<'t>: Iterator<Item = (entity::TempRef<'t, A>, &'t C)>
    where
        Self: 't;
    /// Iterates over all components of a specific discriminant.
    ///
    /// Note that the initializer is not called for lazy-initialized isotope components.
    /// To avoid confusing behavior, do not use this function if [`C: comp::Must<A>`](comp::Must).
    fn iter(&mut self, discrim: K) -> Self::Iter<'_>;

    /// Return value of [`split`](Self::split).
    type Split<'t>: Read<A, C> + 't
    where
        Self: 't;
    /// Splits the accessor into multiple [`Read`] implementors
    /// so that they can be used independently.
    fn split<const N: usize>(&mut self, keys: [K; N]) -> [Self::Split<'_>; N];
}

/// Provides access to an isotope component in a specific archetype
/// without requiring a mutable receiver.
/// Only available for partial storages.
pub trait ReadIsotopeRef<A: Archetype, C: comp::Isotope<A>, K>: ReadIsotope<A, C, K>
where
    K: fmt::Debug + Copy + 'static,
{
    /// Retrieves the component for the given entity and discriminant.
    ///
    /// Identical to [`ReadIsotope::get`] but does not require a mutable receiver.
    fn get_ref<E: entity::Ref<Archetype = A>>(&self, entity: E, key: K) -> &C
    where
        C: comp::Must<A>,
    {
        match self.try_get_ref(entity, key) {
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
    ///
    /// Identical to [`ReadIsotope::try_get`] but does not require a mutable receiver.
    fn try_get_ref<E: entity::Ref<Archetype = A>>(&self, entity: E, key: K) -> Option<&C>;

    /// Return value of [`iter_ref`](Self::iter_ref).
    type IterRef<'t>: Iterator<Item = (entity::TempRef<'t, A>, &'t C)>
    where
        Self: 't;
    /// Iterates over all components of a specific discriminant.
    ///
    /// Identical to [`ReadIsotope::iter`] but does not require a mutable receiver.
    fn iter_ref(&self, key: K) -> Self::IterRef<'_>;
}

/// Provides access to an isotope component in a specific archetype.
pub trait WriteIsotope<A: Archetype, C: comp::Isotope<A>, K = <C as comp::Isotope<A>>::Discrim>:
    ReadIsotope<A, C, K>
where
    K: fmt::Debug + Copy + 'static,
{
    /// Retrieves the component for the given entity and discriminant.
    ///
    /// This method is infallible for correctly implemented `comp::Must`,
    /// which returns the auto-initialized value for missing components.
    fn get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E, discrim: K) -> &mut C
    where
        C: comp::Must<A>,
    {
        match self.try_get_mut(entity, discrim) {
            Some(value) => value,
            None => panic!(
                "{}: comp::Must<{}> but has no default initializer",
                any::type_name::<C>(),
                any::type_name::<A>()
            ),
        }
    }

    /// Returns a mutable reference to the component for the specified entity and discriminant,
    /// automatically initialized with the default initializer if present,
    /// or `None` if the component is unset and has no default initializer.
    ///
    /// Note that this method returns `Option<&mut C>`, not `&mut Option<C>`.
    /// This means setting the Option itself to `Some`/`None` will not modify any stored value.
    /// Use [`WriteIsotope::set`] to add/remove a component.
    fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E, key: K) -> Option<&mut C>;

    /// Overwrites the component for the specified entity and discriminant.
    ///
    /// Passing `None` to this method removes the component from the entity.
    fn set<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        key: K,
        value: Option<C>,
    ) -> Option<C>;

    /// Return value of [`iter_mut`](Self::iter_mut).
    type IterMut<'t>: Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)>
    where
        Self: 't;
    /// Iterates over mutable references to all components of a specific discriminant.
    fn iter_mut(&mut self, key: K) -> Self::IterMut<'_>;

    /// Return value of [`split_isotopes`](Self::split_isotopes).
    type SplitDiscrim<'t>: Write<A, C> + 't
    where
        Self: 't;
    /// Splits the accessor into multiple [`Write`] implementors
    /// so that they can be used in entity iteration independently.
    fn split_isotopes<const N: usize>(&mut self, keys: [K; N]) -> [Self::SplitDiscrim<'_>; N];
}
