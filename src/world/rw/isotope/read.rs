use std::any::type_name;
use std::marker::PhantomData;
use std::sync::Arc;
use std::{fmt, ops};

use parking_lot::lock_api::ArcRwLockReadGuard;
use parking_lot::RwLock;
use rayon::prelude::ParallelIterator;

use crate::entity::ealloc;
use crate::storage::Chunked as _;
use crate::world::rw::isotope;
use crate::{comp, entity, storage, system, Archetype, Storage as _};

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

pub(super) trait StorageGetRef<A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
    Self: isotope::read::StorageGet<A, C>,
{
    fn get_storage_ref(&self, key: Self::Key) -> &C::Storage;
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

    type KnownDiscrims<'t> = impl Iterator<Item = <C as comp::Isotope<A>>::Discrim> + 't where
        Self: 't;
    fn known_discrims(&self) -> Self::KnownDiscrims<'_> {
        self.getter.iter_keys().map(|(_key, discrim)| discrim)
    }

    type GetAll<'t> = impl Iterator<Item = (C::Discrim, &'t C)> + 't where Self: 't;
    fn get_all<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Self::GetAll<'_> {
        // workaround for https://github.com/rust-lang/rust/issues/65442
        fn without_e<A, C>(
            getter: &impl StorageGet<A, C>,
            id: A::RawEntity,
        ) -> impl Iterator<Item = (C::Discrim, &'_ C)> + '_
        where
            A: Archetype,
            C: comp::Isotope<A>,
        {
            getter
                .iter_values()
                .filter_map(move |(_key, discrim, storage)| Some((discrim, storage.get(id)?)))
        }

        without_e(&self.getter, entity.id())
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

impl<A, C, GetterT> system::ReadIsotopeRef<A, C, GetterT::Key> for isotope::Base<GetterT>
where
    A: Archetype,
    C: comp::Isotope<A>,
    GetterT: StorageGetRef<A, C>,
{
    fn try_get_ref<E: entity::Ref<Archetype = A>>(
        &self,
        entity: E,
        key: GetterT::Key,
    ) -> Option<&C> {
        let storage = self.getter.get_storage_ref(key);
        storage.get(entity.id())
    }

    type IterRef<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t C)> where Self: 't;
    fn iter_ref(&self, key: GetterT::Key) -> Self::IterRef<'_> {
        let storage = self.getter.get_storage_ref(key);
        storage.iter().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
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

    type ParIter<'t> = impl rayon::iter::ParallelIterator<Item = (entity::TempRef<'t, A>, &'t C)> where Self: 't, C: comp::Must<A>;
    fn par_iter<'t>(
        &'t self,
        snapshot: &'t ealloc::Snapshot<<A as Archetype>::RawEntity>,
    ) -> Self::ParIter<'t>
    where
        C: comp::Must<A>,
    {
        rayon::iter::split(snapshot.as_slice(), |slice| slice.split()).flat_map_iter(|slice| {
            slice.iter_chunks().flat_map(<A::RawEntity as entity::Raw>::range).map(|id| {
                let entity = entity::TempRef::new(id);
                let data = self.get(entity);
                (entity, data)
            })
        })
    }
}

impl<'u, A, C> system::ReadChunk<A, C> for SplitReader<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A> + comp::Must<A>,
    C::Storage: storage::Chunked,
{
    fn get_chunk(&self, chunk: entity::TempRefChunk<'_, A>) -> &[C] {
        self.storage.get_chunk(chunk.start, chunk.end).expect("chunk is not completely filled")
    }

    type ParIterChunks<'t> = impl rayon::iter::ParallelIterator<Item = (entity::TempRefChunk<'t, A>, &'t [C])> where Self: 't;
    fn par_iter_chunks<'t>(
        &'t self,
        snapshot: &'t ealloc::Snapshot<A::RawEntity>,
    ) -> Self::ParIterChunks<'t> {
        rayon::iter::split(snapshot.as_slice(), |slice| slice.split()).flat_map_iter(|slice| {
            // we don't need to split over the holes in parallel,
            // because splitting the total space is more important than splitting the holes
            slice.iter_chunks().map(|chunk| {
                let chunk = entity::TempRefChunk::new(chunk.start, chunk.end);
                let data = self.get_chunk(chunk);
                (chunk, data)
            })
        })
    }
}
