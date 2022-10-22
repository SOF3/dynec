use std::any::{self, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::{fmt, ops};

use parking_lot::lock_api::ArcRwLockWriteGuard;
use parking_lot::{RawRwLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::storage::Storage;
use super::typed;
use crate::comp::{discrim, Discrim};
use crate::util::DbgTypeId;
use crate::{comp, entity, storage, system, Archetype};

type LockedIsotopeStorage<A, C> = ArcRwLockWriteGuard<RawRwLock, <C as comp::Isotope<A>>::Storage>;

/// Stores the component states in a world.
pub struct Components {
    pub(in crate::world) archetypes: HashMap<DbgTypeId, Box<dyn typed::AnyTyped>>,
}

impl Components {
    /// Creates a dummy, empty component store used for testing.
    pub fn empty() -> Self { Self { archetypes: HashMap::new() } }

    /// Fetches the [`Typed`](typed::Typed) for the requested archetype.
    pub(crate) fn archetype<A: Archetype>(&self) -> &typed::Typed<A> {
        match self.archetypes.get(&DbgTypeId::of::<A>()) {
            Some(typed) => typed.as_any().downcast_ref().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                any::type_name::<A>()
            ),
        }
    }

    /// Fetches the [`Typed`](typed::Typed) for the requested archetype.
    pub(crate) fn archetype_mut<A: Archetype>(&mut self) -> &mut typed::Typed<A> {
        match self.archetypes.get_mut(&DbgTypeId::of::<A>()) {
            Some(typed) => typed.as_any_mut().downcast_mut().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                any::type_name::<A>()
            ),
        }
    }

    /// Creates a read-only, shared accessor to the given archetyped simple component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems
    /// - if another thread is exclusively accessing the same archetyped component.
    pub fn read_simple_storage<A: Archetype, C: comp::Simple<A>>(
        &self,
    ) -> impl system::ReadSimple<A, C> + '_ {
        let storage = match self.archetype::<A>().simple_storages.get(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };
        let guard = match storage.storage.try_read() {
            Some(guard) => guard,
            None => panic!(
                "The component {}/{} is currently exclusively locked by another system. Maybe \
                 scheduler bug?",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };
        let guard = RwLockReadGuard::map(guard, |storage| storage.downcast_ref::<C>());

        SimpleAccessor { storage: guard }
    }

    /// Creates a writable, exclusive accessor to the given archetyped simple component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is accessing the same archetyped component.
    pub fn write_simple_storage<A: Archetype, C: comp::Simple<A>>(
        &self,
    ) -> impl system::WriteSimple<A, C> + '_ {
        let storage = match self.archetype::<A>().simple_storages.get(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };
        let guard = match storage.storage.try_write() {
            Some(guard) => guard,
            None => panic!(
                "The component {}/{} is currently used by another system. Maybe scheduler bug?",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };
        let guard = RwLockWriteGuard::map(guard, |storage| storage.downcast_mut::<C>());

        SimpleAccessor { storage: guard }
    }

    fn get_isotope_storage_map<A: Archetype, C: comp::Isotope<A>>(
        &self,
    ) -> &storage::IsotopeMap<A, C> {
        match self.archetype::<A>().isotope_storage_maps.get(&TypeId::of::<C>()) {
            Some(storage) => storage.downcast_ref::<C>(),
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        }
    }

    /// Creates a read-only, shared accessor to all discriminants of the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is exclusively accessing any discriminants of the isotope component.
    pub fn read_full_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
        &self,
    ) -> impl system::ReadIsotope<A, C> + '_ {
        let storage_map = self.get_isotope_storage_map::<A, C>();

        let storages: <C::Discrim as Discrim>::FullMap<_> = {
            // note: lock contention may occur here if another thread is requesting write access to
            // storages of other discriminants.
            let map = storage_map.map.read();

            map.iter()
                .map(|(&discrim, storage)| {
                    (discrim, own_read_isotope_storage::<A, C>(discrim, storage))
                })
                .collect()
        };

        struct Proc<S>(PhantomData<S>);
        impl<S> StorageMapProcessorRef for Proc<S> {
            type Input = S;
            type Output = S;
            fn process<'t, D: fmt::Debug, F: FnOnce() -> D>(
                &self,
                input: Option<&'t S>,
                _: F,
            ) -> Option<&'t S> {
                input
            }
            fn admit(input: &S) -> Option<&S> { Some(input) }
        }

        IsotopeAccessor { storages, processor: Proc(PhantomData), _ph: PhantomData }
    }

    /// Creates a read-only, shared accessor to specific discriminants of the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is exclusively accessing any of the requested discriminants.
    pub fn read_partial_isotope_storage<'t, A, C, T>(
        &'t self,
        discrims: &'t T,
    ) -> impl system::ReadIsotope<A, C, T::Key> + 't
    where
        A: Archetype,
        C: comp::Isotope<A>,
        T: discrim::Set<C::Discrim>,
    {
        let storage_map = self.get_isotope_storage_map::<A, C>();

        let storages: T::Mapped<Option<_>> = {
            // note: lock contention may occur here if another thread is requesting write access to
            // storages of other discriminants.
            let map = storage_map.map.read();

            discrims
                .map(|discrim| Some(own_read_isotope_storage::<A, C>(discrim, map.get(&discrim)?)))
        };

        struct Proc<A, C, S>(PhantomData<(A, C, S)>);
        impl<A, C, S> StorageMapProcessorRef for Proc<A, C, S> {
            type Input = Option<S>;
            type Output = S;

            fn process<'t, D: fmt::Debug, F: FnOnce() -> D>(
                &self,
                input: Option<&'t Option<S>>,
                key: F,
            ) -> Option<&'t S> {
                match input {
                    Some(Some(storage)) => Some(storage), // already initialized
                    Some(None) => None, // valid discriminant, but not yet initialized
                    None => panic_invalid_key::<A, C>(key()),
                }
            }

            fn admit(input: &Option<S>) -> Option<&S> { input.as_ref() }
        }

        IsotopeAccessor { storages, processor: Proc::<A, C, _>(PhantomData), _ph: PhantomData }
    }

    /// Creates a writable, exclusive accessor to all discriminants of the given archetyped isotope component,
    /// with the capability of initializing creating new discriminants not previously created.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is accessing the same archetyped component.
    pub fn write_full_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
        &self,
    ) -> impl system::WriteIsotope<A, C> + '_ {
        let storage_map = self.get_isotope_storage_map::<A, C>();

        let full_map: RwLockWriteGuard<'_, storage::IsotopeMapInner<A, C>> =
            storage_map.map.write();

        let accessor_storages: <C::Discrim as Discrim>::FullMap<LockedIsotopeStorage<A, C>> =
            full_map
                .iter()
                .map(|(&discrim, storage)| {
                    (discrim, own_write_isotope_storage::<A, C>(discrim, storage))
                })
                .collect();

        struct Proc<'t, A, C>
        where
            A: Archetype,
            C: comp::Isotope<A>,
        {
            full_map: RwLockWriteGuard<'t, storage::IsotopeMapInner<A, C>>,
            _ph:      PhantomData<(A, C)>,
        }
        impl<'t, A, C> StorageMapProcessorRef for Proc<'t, A, C>
        where
            A: Archetype,
            C: comp::Isotope<A>,
        {
            type Input = LockedIsotopeStorage<A, C>;
            type Output = Self::Input;
            fn process<'u, D: fmt::Debug, F: FnOnce() -> D>(
                &self,
                input: Option<&'u Self::Input>,
                _: F,
            ) -> Option<&'u Self::Input> {
                input
            }
            fn admit(input: &Self::Input) -> Option<&Self::Input> { Some(input) }
        }
        impl<'t, A, C, M> MutStorageAccessor<A, C, LockedIsotopeStorage<A, C>, M> for Proc<'t, A, C>
        where
            A: Archetype,
            C: comp::Isotope<A>,
            M: discrim::FullMap<
                Discrim = C::Discrim,
                Key = C::Discrim,
                Value = LockedIsotopeStorage<A, C>,
            >,
        {
            fn get_storage<'u>(
                &mut self,
                discrim: C::Discrim,
                storages: &'u mut M,
            ) -> &'u mut C::Storage
            where
                LockedIsotopeStorage<A, C>: 'u,
            {
                storages.get_by_or_insert(discrim, || {
                    let arc = Arc::default();
                    let ret = own_write_isotope_storage::<A, C>(discrim, &arc);
                    self.full_map.insert(discrim, arc);
                    ret
                })
            }
        }

        IsotopeAccessor::<A, C, LockedIsotopeStorage<A, C>, _, _> {
            storages:  accessor_storages,
            processor: Proc::<'_, A, C> { full_map, _ph: PhantomData },
            _ph:       PhantomData,
        }
    }

    /// Creates a writable, exclusive accessor to the given archetyped isotope component,
    /// initializing new discriminants if not previously created.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems
    /// - if another thread is accessing the same archetyped component.
    pub fn write_partial_isotope_storage<
        't,
        A: Archetype,
        C: comp::Isotope<A>,
        S: discrim::Set<C::Discrim>,
    >(
        &'t self,
        discrims: &'t S,
    ) -> impl system::WriteIsotope<A, C, S::Key> + 't {
        let storage_map = self.get_isotope_storage_map::<A, C>();

        let storages = {
            // note: lock contention may occur here if another thread is requesting write access to
            // storages of other discriminants.
            let mut map = storage_map.map.write();

            discrims.map(|discrim| {
                let storage = map.entry(discrim).or_insert_with(Arc::<RwLock<C::Storage>>::default);
                own_write_isotope_storage::<A, C>(discrim, storage)
            })
        };

        struct Proc<A, C, S>(PhantomData<(A, C, S)>);
        impl<A, C, S> StorageMapProcessorRef for Proc<A, C, S> {
            type Input = S;
            type Output = S;

            fn process<'t, D: fmt::Debug, F: FnOnce() -> D>(
                &self,
                input: Option<&'t S>,
                key: F,
            ) -> Option<&'t S> {
                match input {
                    Some(input) => Some(input),
                    None => panic_invalid_key::<A, C>(key()),
                }
            }

            fn admit(input: &Self::Input) -> Option<&Self::Output> { Some(input) }
        }
        impl<A, C, S, M> MutStorageAccessor<A, C, S, M> for Proc<A, C, S>
        where
            A: Archetype,
            C: comp::Isotope<A>,
            S: ops::DerefMut<Target = C::Storage>,
            M: discrim::Mapped<Discrim = C::Discrim, Value = S>,
        {
            fn get_storage<'t>(&mut self, key: M::Key, storages: &'t mut M) -> &'t mut C::Storage
            where
                S: 't,
            {
                match storages.get_mut_by(key).map(|s| &mut **s) {
                    Some(storage) => storage,
                    None => panic!(
                        "Cannot access isotope indexed by {key:?} because it is not in the list \
                         of requested discriminants",
                    ),
                }
            }
        }

        IsotopeAccessor { storages, processor: Proc(PhantomData), _ph: PhantomData }
    }
}

struct SimpleAccessor<S> {
    storage: S,
}

impl<A, C, S> system::Read<A, C> for SimpleAccessor<S>
where
    A: Archetype,
    C: comp::Simple<A>,
    S: ops::Deref<Target = C::Storage>,
{
    type Get<'t> = &'t C where S: 't;
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C> {
        self.storage.get(entity.id())
    }

    type Iter<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t C)> where Self: 't;
    fn iter(&self) -> Self::Iter<'_> {
        self.storage.iter().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }
}
impl<A, C, S> system::ReadSimple<A, C> for SimpleAccessor<S>
where
    A: Archetype,
    C: comp::Simple<A>,
    S: ops::Deref<Target = C::Storage>,
{
}

impl<A, C, S> system::Write<A, C> for SimpleAccessor<S>
where
    A: Archetype,
    C: comp::Simple<A>,
    S: ops::DerefMut<Target = C::Storage>,
{
    fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C> {
        self.storage.get_mut(entity.id())
    }

    fn set<E: entity::Ref<Archetype = A>>(&mut self, entity: E, value: Option<C>) -> Option<C> {
        self.storage.set(entity.id(), value)
    }

    type IterMut<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)> where Self: 't;
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.storage.iter_mut().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }
}
impl<A, C, S> system::WriteSimple<A, C> for SimpleAccessor<S>
where
    A: Archetype,
    C: comp::Simple<A>,
    S: ops::DerefMut<Target = C::Storage>,
{
}

fn own_read_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
    discrim: C::Discrim,
    storage: &Arc<RwLock<C::Storage>>,
) -> impl ops::Deref<Target = C::Storage> {
    let storage: Arc<RwLock<C::Storage>> = Arc::clone(storage);
    match storage.try_read_arc() {
        Some(guard) => guard,
        None => panic!(
            "The component {}/{}/{:?} is currently used by another system. Maybe scheduler bug?",
            any::type_name::<A>(),
            any::type_name::<C>(),
            discrim,
        ),
    }
}

fn own_write_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
    discrim: C::Discrim,
    storage: &Arc<RwLock<C::Storage>>,
) -> LockedIsotopeStorage<A, C> {
    let storage: Arc<RwLock<C::Storage>> = Arc::clone(storage);
    match storage.try_write_arc() {
        Some(guard) => guard,
        None => panic!(
            "The component {}/{}/{:?} is currently used by another system. Maybe scheduler bug?",
            any::type_name::<A>(),
            any::type_name::<C>(),
            discrim,
        ),
    }
}

struct IsotopeAccessor<A, C, S, M, P> {
    /// Cloned arcs of the actual storage.
    storages:  M,
    processor: P,
    _ph:       PhantomData<(A, C, S)>,
}

fn panic_invalid_key<A, C>(key: impl fmt::Debug) -> ! {
    panic!(
        "The index {key:?} is not available in the isotope request for {}/{}",
        any::type_name::<A>(),
        any::type_name::<C>(),
    )
}

impl<A, C, S, M, P> IsotopeAccessor<A, C, S, M, P>
where
    A: Archetype,
    C: comp::Isotope<A>,
    S: ops::Deref<Target = C::Storage>,
    M: discrim::Mapped<Discrim = C::Discrim>,
    P: StorageMapProcessorRef<Input = M::Value, Output = S>,
{
    fn get_all_raw(
        &self,
        index: A::RawEntity,
    ) -> impl Iterator<Item = (<C as comp::Isotope<A>>::Discrim, &C)> {
        self.storages.iter_values().filter_map(move |(discrim, storage)| {
            let storage = P::admit(storage)?;
            let comp = storage.get(index)?;
            Some((discrim, comp))
        })
    }
}

trait StorageMapProcessorRef {
    type Input;
    type Output;

    fn process<'t, D: fmt::Debug, F: FnOnce() -> D>(
        &self,
        input: Option<&'t Self::Input>,
        key: F,
    ) -> Option<&'t Self::Output>;
    fn admit(input: &Self::Input) -> Option<&Self::Output>;
}

impl<A, C, S, M, P> system::ReadIsotope<A, C, M::Key> for IsotopeAccessor<A, C, S, M, P>
where
    A: Archetype,
    C: comp::Isotope<A>,
    S: ops::Deref<Target = C::Storage>,
    M: discrim::Mapped<Discrim = C::Discrim>,
    P: StorageMapProcessorRef<Input = M::Value, Output = S>,
{
    type Get<'t> = &'t C where Self: 't;
    fn try_get<E: entity::Ref<Archetype = A>>(
        &self,
        entity: E,
        key: M::Key,
    ) -> Option<Self::Get<'_>> {
        let storage: Option<&M::Value> = self.storages.get_by(key);
        let storage: Option<&S> = self.processor.process(storage, || key);
        storage.and_then(|storage| storage.get(entity.id()))
    }

    type GetAll<'t> = impl Iterator<Item = (<C as comp::Isotope<A>>::Discrim, &'t C)> + 't where Self: 't;
    fn get_all<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Self::GetAll<'_> {
        let index: A::RawEntity = entity.id();
        self.get_all_raw(index)
    }

    type Iter<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t C)>
    where
        Self: 't;
    fn iter(&self, key: M::Key) -> Self::Iter<'_> {
        let storage: Option<&M::Value> = self.storages.get_by(key);
        let storage: Option<&S> = self.processor.process(storage, || key);
        storage.into_iter().flat_map(|storage| {
            storage.iter().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
        })
    }
}

trait MutStorageAccessor<A, C, S, M>
where
    A: Archetype,
    C: comp::Isotope<A>,
    S: ops::Deref<Target = C::Storage>,
    M: discrim::Mapped<Discrim = C::Discrim>,
{
    fn get_storage<'t>(&mut self, key: M::Key, storages: &'t mut M) -> &'t mut C::Storage
    where
        S: 't;
}

impl<A, C, S, M, P> system::WriteIsotope<A, C, M::Key> for IsotopeAccessor<A, C, S, M, P>
where
    A: Archetype,
    C: comp::Isotope<A>,
    S: ops::Deref<Target = C::Storage>,
    M: discrim::Mapped<Discrim = C::Discrim>,
    P: StorageMapProcessorRef<Input = M::Value, Output = S> + MutStorageAccessor<A, C, S, M>,
{
    fn try_get_mut<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        key: M::Key,
    ) -> Option<&mut C> {
        let storage = self.processor.get_storage(key, &mut self.storages);

        storage.get_mut(entity.id())
    }

    fn set<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        key: M::Key,
        value: Option<C>,
    ) -> Option<C> {
        let storage = self.processor.get_storage(key, &mut self.storages);
        storage.set(entity.id(), value)
    }

    type IterMut<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)>
    where
        Self: 't;
    /// Iterates over mutable references to all components of a specific discriminant.
    fn iter_mut(&mut self, key: M::Key) -> Self::IterMut<'_> {
        let storage = self.processor.get_storage(key, &mut self.storages);
        storage.iter_mut().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }
}

#[cfg(test)]
static_assertions::assert_impl_all!(Components: Send, Sync);
