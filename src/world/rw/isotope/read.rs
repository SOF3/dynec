use std::any::type_name;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use parking_lot::lock_api::ArcRwLockReadGuard;
use parking_lot::RwLock;

use crate::world::rw::isotope;
use crate::{comp, entity, system, Archetype, Storage as _};

pub(super) mod full;
pub(super) mod partial;

type LockedStorage<A, C> =
    ArcRwLockReadGuard<parking_lot::RawRwLock, <C as comp::SimpleOrIsotope<A>>::Storage>;

fn own_storage<A: Archetype, C: comp::Isotope<A>>(
    discrim: C::Discrim,
    storage: &Arc<RwLock<C::Storage>>,
) -> LockedStorage<A, C> {
    let storage: Arc<RwLock<C::Storage>> = Arc::clone(storage);
    match storage.try_read_arc() {
        Some(guard) => guard,
        None => panic!(
            "The component {}/{}/{:?} is currently uniquely locked by another system. Maybe \
             scheduler bug?",
            type_name::<A>(),
            type_name::<C>(),
            discrim,
        ),
    }
}

/// Abstracts the storage access pattern for an accessor type.
pub(super) trait StorageGet<A, C>
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
    fn iter_keys(&mut self) -> Self::IterKeys<'_>;
}

impl<A, C, GetterT> system::ReadIsotope<A, C, GetterT::Key> for isotope::Base<GetterT>
where
    A: Archetype,
    C: comp::Isotope<A>,
    GetterT: StorageGet<A, C>,
{
    fn try_get<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        key: GetterT::Key,
    ) -> Option<&C> {
        let storage = self.getter.get_storage(key);
        storage.get(entity.id())
    }

    type GetAll<'t> = impl Iterator<Item = <C as comp::Isotope<A>>::Discrim> + 't where
        Self: 't;
    fn get_all(&mut self) -> Self::GetAll<'_> {
        self.getter.iter_keys().map(|(_key, discrim)| discrim)
    }

    type Iter<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t C)>
    where
        Self: 't;

    fn iter(&mut self, key: GetterT::Key) -> Self::Iter<'_> {
        let storage = self.getter.get_storage(key);
        storage.iter().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }

    type Split<'t> = impl system::Read<A, C> + 't
    where
        Self: 't;

    fn split<const N: usize>(&mut self, keys: [GetterT::Key; N]) -> [Self::Split<'_>; N] {
        let storages = self.getter.get_storage_many(keys);
        storages.map(|storage| SplitReader { storage, _ph: PhantomData })
    }
}

pub(super) struct SplitReader<'t, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    pub(super) storage: &'t C::Storage,
    pub(super) _ph:     PhantomData<(A, C)>,
}

impl<'u, A, C> system::Read<A, C> for SplitReader<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C> {
        self.storage.get(entity.id())
    }

    type Iter<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t C)> where Self: 't;

    fn iter(&self) -> Self::Iter<'_> {
        self.storage.iter().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }

    type DuplicateImmut<'t> = impl system::Read<A, C> + 't where Self: 't;

    fn duplicate_immut(&self) -> (Self::DuplicateImmut<'_>, Self::DuplicateImmut<'_>) {
        (
            Self { storage: self.storage, _ph: PhantomData },
            Self { storage: self.storage, _ph: PhantomData },
        )
    }
}
