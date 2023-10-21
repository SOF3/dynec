//! Traits for accessing a single component storage.
//!
//! See [`AccessIsotope`](Isotope) for documentation.

use std::marker::PhantomData;
use std::{any, fmt, ops};

use derive_trait::derive_trait;

use crate::storage::Access as _;
use crate::system::AccessSingle;
use crate::{comp, entity, Archetype, Storage as _};

/// Accesses multiple storages for the same isotope.
pub struct Isotope<A, C, StorageMapT> {
    storages: StorageMapT,
    _ph:      PhantomData<(A, C)>,
}

impl<A, C, StorageMapT> Isotope<A, C, StorageMapT> {
    pub(crate) fn new(storages: StorageMapT) -> Self { Self { storages, _ph: PhantomData } }
}

/// Implements the access pattern for multiple isotope storages.
pub trait StorageMap<A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    /// The key from the user, equivalent to [`comp::discrim::Set::Key`]
    type Key: fmt::Debug + Copy + 'static;

    /// Retrieves a storage by key.
    /// Panics if the key is not supported.
    ///
    /// For partial accessors, this should return the storage
    /// for the discriminant indexed by the key,
    /// or panic if the key is out of bounds.
    ///
    /// For full accessors, this should return the storage for the given discriminant,
    /// or initialize the storage lazily.
    fn get_storage(&mut self, key: Self::Key) -> &C::Storage;

    /// Equivalent to calling [`Self::get_storage`] for each key.
    ///
    /// Duplicate keys are allowed because the return type is immutable.
    /// The mutability is only used for lazy initialization.
    fn get_storage_many<const N: usize>(&mut self, keys: [Self::Key; N]) -> [&C::Storage; N];

    /// Return value of [`iter_keys`](Self::iter_keys).
    type IterKeys<'t>: Iterator<Item = (Self::Key, C::Discrim)> + 't
    where
        Self: 't;
    /// Iterates over all keys currently accessible from this accessor.
    ///
    /// For partial accessors, this is the set of keys to the discriminants provided by the user.
    ///
    /// For full accessors, this is the set of discriminants that have been initialized.
    fn iter_keys(&self) -> Self::IterKeys<'_>;

    /// Storage type yielded by [`iter_values`](Self::iter_values).
    type IterValue: ops::Deref<Target = C::Storage>;
    /// Return value of [`iter_values`](Self::iter_values).
    type IterValues<'t>: Iterator<Item = (Self::Key, C::Discrim, &'t Self::IterValue)> + 't
    where
        Self: 't;
    /// Iterates over all storages currently accessible from this accessor.
    ///
    /// For partial accessors, this is the set of keys to the discriminants provided by the user.
    ///
    /// For full accessors, this is the set of discriminants that have been initialized.
    fn iter_values(&self) -> Self::IterValues<'_>;
}

/// Like [`StorageMap`] but can access a storage without `&mut self`.
///
/// Only available for partial accessors,
/// because a full accessor needs to mutate its local copy of storage map.
pub trait PartialStorageMap<A, C>: StorageMap<A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    /// Retrieves a storage by key like [`get_storage`](StorageMap::get_storage),
    /// but without exclusively borrowing the accessor.
    fn get_storage_ref(&self, key: Self::Key) -> &C::Storage;
}

/// Implements the access pattern for multiple isotope storages.
pub trait StorageMapMut<A, C>: StorageMap<A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    /// Retrieves a storage by key.
    /// Panics if the key is not supported.
    ///
    /// For partial accessors, this should return the storage
    /// for the discriminant indexed by the key,
    /// or panic if the key is out of bounds.
    ///
    /// For full accessors, this should return the storage for the given discriminant,
    /// or initialize the storage lazily.
    fn get_storage_mut(&mut self, key: Self::Key) -> &mut C::Storage;

    /// Retrieves storages by disjoint keys.
    /// Panics if any key is not supported or is equal to another key.
    fn get_storage_mut_many<const N: usize>(
        &mut self,
        keys: [Self::Key; N],
    ) -> [&mut C::Storage; N];
}

#[derive_trait(pub Get{
    /// The archetype that this accessor retrieves for.
    type Arch: Archetype = A;
    /// The component that this accessor retrieves.
    type Comp: comp::Isotope<Self::Arch> = C;
    /// The key for the discriminant set, `Comp::Discrim` for full accessors, typically `usize` for partial accessors
    type Key: fmt::Debug + Copy + 'static = KeyT;
})]
impl<A, C, KeyT, StorageMapT> Isotope<A, C, StorageMapT>
where
    A: Archetype,
    C: comp::Isotope<A>,
    KeyT: fmt::Debug + Copy + 'static,
    StorageMapT: StorageMap<A, C, Key = KeyT>,
{
    /// Retrieves the component for the given entity and discriminant.
    ///
    /// This method is infallible for correctly implemented `comp::Must`,
    /// which returns the auto-initialized value for missing components.
    pub fn get(&mut self, entity: impl entity::Ref<Archetype = A>, discrim: KeyT) -> &C
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
    pub fn try_get(&mut self, entity: impl entity::Ref<Archetype = A>, key: KeyT) -> Option<&C> {
        let storage = self.storages.get_storage(key);
        storage.get(entity.id())
    }

    /// Iterates over all known discriminants of the component type.
    ///
    /// The yielded discriminants are not in any guaranteed order.
    pub fn known_discrims<'t>(
        &'t self,
    ) -> impl Iterator<Item = <C as comp::Isotope<A>>::Discrim> + 't {
        self.storages.iter_keys().map(|(_key, discrim)| discrim)
    }

    /// Iterates over all known isotopes for a specific entity.
    pub fn get_all<'t, E: entity::Ref<Archetype = A>>(
        &'t self,
        entity: E,
    ) -> impl Iterator<Item = (<C as comp::Isotope<A>>::Discrim, &'t C)> + 't {
        // workaround for https://github.com/rust-lang/rust/issues/65442
        fn without_e<A, C>(
            getter: &impl StorageMap<A, C>,
            id: <A as Archetype>::RawEntity,
        ) -> impl Iterator<Item = (C::Discrim, &'_ C)> + '_
        where
            A: Archetype,
            C: comp::Isotope<A>,
        {
            getter
                .iter_values()
                .filter_map(move |(_key, discrim, storage)| Some((discrim, storage.get(id)?)))
        }

        without_e(&self.storages, entity.id())
    }

    /// Iterates over all components of a specific discriminant.
    ///
    /// Note that the initializer is not called for lazy-initialized isotope components.
    /// To avoid confusing behavior, do not use this function if [`C: comp::Must<A>`](comp::Must).
    pub fn iter<'t>(
        &'t mut self,
        key: KeyT,
    ) -> impl Iterator<Item = (entity::TempRef<'t, A>, &'t C)> {
        let storage = self.storages.get_storage(key);
        storage.iter().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }

    /// Splits the accessor into multiple immutable [`AccessSingle`] accessors
    /// so that they can be used independently.
    pub fn split<'t, const N: usize>(
        &'t mut self,
        keys: [KeyT; N],
    ) -> [AccessSingle<A, C, impl ops::Deref<Target = <C as comp::SimpleOrIsotope<A>>::Storage> + 't>;
           N] {
        let storages = self.storages.get_storage_many(keys);
        storages.map(|storage| AccessSingle::new(storage))
    }
}

#[derive_trait(pub GetRef{
    /// The archetype that this accessor retrieves for.
    type Arch: Archetype = A;
    /// The component that this accessor retrieves.
    type Comp: comp::Isotope<Self::Arch> = C;
    /// The key for the discriminant set, `Comp::Discrim` for full accessors, typically `usize` for partial accessors
    type Key: fmt::Debug + Copy + 'static = KeyT;
})]
impl<A, C, KeyT, StorageMapT> Isotope<A, C, StorageMapT>
where
    A: Archetype,
    C: comp::Isotope<A>,
    KeyT: fmt::Debug + Copy + 'static,
    StorageMapT: PartialStorageMap<A, C, Key = KeyT>,
{
    /// Retrieves the component for the given entity and discriminant.
    ///
    /// Identical to [`get`](Isotope::get) but does not require a mutable receiver.
    pub fn get_ref(&self, entity: impl entity::Ref<Archetype = A>, key: KeyT) -> &C
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
    /// Identical to [`try_get`](Isotope::try_get) but does not require a mutable receiver.
    pub fn try_get_ref<E: entity::Ref<Archetype = A>>(&self, entity: E, key: KeyT) -> Option<&C> {
        let storage = self.storages.get_storage_ref(key);
        storage.get(entity.id())
    }

    /// Iterates over all components of a specific discriminant.
    ///
    /// Identical to [`iter`](Isotope::iter) but does not require a mutable receiver.
    pub fn iter_ref<'t>(
        &'t self,
        key: KeyT,
    ) -> impl Iterator<Item = (entity::TempRef<'t, A>, &'t C)> {
        let storage = self.storages.get_storage_ref(key);
        storage.iter().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }
}

#[derive_trait(pub GetMut{
    /// The archetype that this accessor retrieves for.
    type Arch: Archetype = A;
    /// The component that this accessor retrieves.
    type Comp: comp::Isotope<Self::Arch> = C;
    /// The key for the discriminant set, `Comp::Discrim` for full accessors, typically `usize` for partial accessors
    type Key: fmt::Debug + Copy + 'static = KeyT;
})]
impl<A, C, KeyT, StorageMapT> Isotope<A, C, StorageMapT>
where
    A: Archetype,
    C: comp::Isotope<A>,
    KeyT: fmt::Debug + Copy + 'static,
    StorageMapT: StorageMapMut<A, C, Key = KeyT>,
{
    /// Retrieves the component for the given entity and discriminant.
    ///
    /// This method is infallible for correctly implemented `comp::Must`,
    /// which returns the auto-initialized value for missing components.
    pub fn get_mut(&mut self, entity: impl entity::Ref<Archetype = A>, discrim: KeyT) -> &mut C
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
    /// Use [`set`](Isotope::set) to add/remove a component.
    pub fn try_get_mut(
        &mut self,
        entity: impl entity::Ref<Archetype = A>,
        key: KeyT,
    ) -> Option<&mut C> {
        let storage = self.storages.get_storage_mut(key);
        storage.get_mut(entity.id())
    }

    /// Overwrites the component for the specified entity and discriminant.
    ///
    /// Passing `None` to this method removes the component from the entity.
    pub fn set<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        key: KeyT,
        value: Option<C>,
    ) -> Option<C> {
        let storage = self.storages.get_storage_mut(key);
        storage.set(entity.id(), value)
    }

    /// Iterates over mutable references to all components of a specific discriminant.
    pub fn iter_mut<'t>(
        &'t mut self,
        key: KeyT,
    ) -> impl Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)> {
        let storage = self.storages.get_storage_mut(key);
        storage.iter_mut().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }

    /// Splits the accessor into multiple mutable [`AccessSingle`] accessors
    /// so that they can be used in entity iteration independently.
    pub fn split_mut<'t, const N: usize>(
        &'t mut self,
        keys: [KeyT; N],
    ) -> [AccessSingle<
        A,
        C,
        impl ops::DerefMut<Target = <C as comp::SimpleOrIsotope<A>>::Storage> + 't,
    >; N] {
        let storages = self.storages.get_storage_mut_many(keys);
        storages.map(|storage| AccessSingle::new(storage))
    }
}

#[cfg(test)]
mod tests;
