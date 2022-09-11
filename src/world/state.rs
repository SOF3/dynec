use std::any::{self, Any, TypeId};
use std::collections::HashMap;
use std::ops;
use std::sync::Arc;

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::storage::Storage;
use super::typed;
use crate::comp::discrim::Map as _;
use crate::comp::Discrim;
use crate::entity::referrer;
use crate::util::DbgTypeId;
use crate::{comp, entity, system, Archetype, Global};

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

        struct Ret<R: ops::Deref> {
            storage: R,
        }

        impl<A: Archetype, C: comp::Simple<A>, R: ops::Deref<Target = C::Storage>>
            system::ReadSimple<A, C> for Ret<R>
        {
            fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C> {
                self.storage.get(entity.id())
            }
        }

        Ret { storage: guard }
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

        struct Ret<R: ops::Deref + ops::DerefMut> {
            storage: R,
        }

        impl<
                A: Archetype,
                C: comp::Simple<A>,
                S: ops::Deref<Target = C::Storage> + ops::DerefMut,
            > system::ReadSimple<A, C> for Ret<S>
        {
            fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C> {
                self.storage.get(entity.id())
            }
        }
        impl<
                A: Archetype,
                C: comp::Simple<A>,
                S: ops::Deref<Target = C::Storage> + ops::DerefMut,
            > system::WriteSimple<A, C> for Ret<S>
        {
            fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C> {
                self.storage.get_mut(entity.id())
            }

            fn set<E: entity::Ref<Archetype = A>>(
                &mut self,
                entity: E,
                value: Option<C>,
            ) -> Option<C> {
                self.storage.set(entity.id(), value)
            }
        }

        Ret { storage: guard }
    }

    /// Creates a read-only, shared accessor to all discriminants of the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is exclusively accessing any discriminants of the isotope component.
    pub fn read_full_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
        &self,
    ) -> impl system::ReadIsotope<A, C> + '_ {
        self.read_isotope_storage(None)
    }

    /// Creates a read-only, shared accessor to specific discriminants of the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is exclusively accessing any of the requested discriminants.
    pub fn read_partial_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
        &self,
        discrims: &[usize],
    ) -> impl system::ReadIsotope<A, C> + '_ {
        self.read_isotope_storage(Some(discrims))
    }

    fn read_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
        &self,
        discrims: Option<&[usize]>,
    ) -> impl system::ReadIsotope<A, C> + '_ {
        let storage_map = match self.archetype::<A>().isotope_storage_maps.get(&TypeId::of::<C>()) {
            Some(storage) => storage.downcast_ref::<C>(),
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };

        let storages: <C::Discrim as comp::Discrim>::Map<_> = {
            // note: lock contention may occur here if another thread is requesting write access to
            // storages of other discriminants.
            let map = storage_map.map.read();

            map.iter()
                .filter(|(&discrim, _)| match discrims {
                    Some(discrims) => discrims.contains(&discrim),
                    None => true,
                })
                .map(|(&discrim, storage)| {
                    let storage: Arc<RwLock<C::Storage>> = Arc::clone(storage);
                    let storage = match storage.try_read_arc() {
                        Some(guard) => guard,
                        None => panic!(
                            "The component {}/{}/{} is currently used by another system. Maybe \
                             scheduler bug?",
                            any::type_name::<A>(),
                            any::type_name::<C>(),
                            discrim,
                        ),
                    };
                    (discrim, storage)
                })
                .collect()
        };

        IsotopeAccessor { storages }
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
        let storage_map = match self.archetype::<A>().isotope_storage_maps.get(&TypeId::of::<C>()) {
            Some(storage) => storage.downcast_ref::<C>(),
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };

        let full_map = storage_map.map.write();

        let accessor_storages: <C::Discrim as comp::Discrim>::Map<_> = full_map
            .iter()
            .map(|(&discrim, storage)| {
                let storage: Arc<RwLock<C::Storage>> = Arc::clone(storage);
                let storage = match storage.try_write_arc() {
                    Some(guard) => guard,
                    None => panic!(
                        "The component {}/{}/{} is currently used by another system. Maybe \
                         scheduler bug?",
                        any::type_name::<A>(),
                        any::type_name::<C>(),
                        discrim,
                    ),
                };
                (discrim, storage)
            })
            .collect();

        FullIsotopeAccessor {
            full_map,
            isotope_accessor: IsotopeAccessor { storages: accessor_storages },
        }
    }

    /// Creates a writable, exclusive accessor to the given archetyped isotope component,
    /// initializing new discriminants if not previously created.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems
    /// - if another thread is accessing the same archetyped component.
    pub fn write_partial_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
        &self,
        discrims: &[usize],
    ) -> impl system::WriteIsotope<A, C> + '_ {
        let storage_map = match self.archetype::<A>().isotope_storage_maps.get(&TypeId::of::<C>()) {
            Some(storage) => storage.downcast_ref::<C>(),
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };

        let storages: <C::Discrim as comp::Discrim>::Map<_> = {
            // note: lock contention may occur here if another thread is requesting write access to
            // storages of other discriminants.
            let mut map = storage_map.map.write();

            discrims
                .iter()
                .map(|&discrim| {
                    let storage =
                        map.entry(discrim).or_insert_with(Arc::<RwLock<C::Storage>>::default);
                    let storage = Arc::clone(storage);

                    let storage = match storage.try_write_arc() {
                        Some(guard) => guard,
                        None => panic!(
                            "The component {}/{}/{} is currently used by another system. Maybe \
                             scheduler bug?",
                            any::type_name::<A>(),
                            any::type_name::<C>(),
                            discrim,
                        ),
                    };

                    (discrim, storage)
                })
                .collect()
        };

        IsotopeAccessor { storages }
    }
}

struct IsotopeAccessor<A: Archetype, C: comp::Isotope<A>, S> {
    storages: <C::Discrim as Discrim>::Map<S>,
}

impl<A: Archetype, C: comp::Isotope<A>, S: ops::Deref<Target = C::Storage>>
    system::ReadIsotope<A, C> for IsotopeAccessor<A, C, S>
{
    type IsotopeRefMap<'t> = impl Iterator<Item = (<C as comp::Isotope<A>>::Discrim, &'t C)> + 't where Self: 't;

    fn get<E: entity::Ref<Archetype = A>>(
        &self,
        entity: E,
        discrim: C::Discrim,
    ) -> system::RefOrDefault<'_, C>
    where
        C: comp::Must<A>,
    {
        match self.try_get(entity, discrim) {
            Some(value) => system::RefOrDefault(system::BorrowedOwned::Borrowed(value)),
            None => system::RefOrDefault(system::BorrowedOwned::Owned(comp::must_isotope_init())),
        }
    }

    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E, discrim: C::Discrim) -> Option<&C> {
        let discrim = discrim.into_usize();

        // if storage does not exist, the component does not exist yet.
        let storage = self.storages.find(discrim)?;

        storage.get(entity.id())
    }

    fn get_all<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Self::IsotopeRefMap<'_> {
        let index: A::RawEntity = entity.id();

        fn filter_map_fn<S: ops::Deref<Target = C::Storage>, A: Archetype, C: comp::Isotope<A>>(
            index: A::RawEntity,
        ) -> impl Fn((usize, &S)) -> Option<(C::Discrim, &C)> {
            move |(discrim, storage)| {
                let discrim = <C::Discrim as comp::Discrim>::from_usize(discrim);
                let comp = storage.get(index)?;
                Some((discrim, comp))
            }
        }

        self.storages.iter().filter_map(filter_map_fn(index))
    }
}

impl<A: Archetype, C: comp::Isotope<A>, S: ops::Deref> system::WriteIsotope<A, C>
    for IsotopeAccessor<A, C, S>
{
    // TODO
}

struct FullIsotopeAccessor<A: Archetype, C: comp::Isotope<A>, M, S> {
    full_map:         M,
    isotope_accessor: IsotopeAccessor<A, C, S>,
}

impl<A, C, M, S> system::ReadIsotope<A, C> for FullIsotopeAccessor<A, C, M, S>
where
    A: Archetype,
    C: comp::Isotope<A>,
    M: ops::Deref<Target = HashMap<usize, Arc<RwLock<C::Storage>>>>,
    IsotopeAccessor<A, C, S>: system::ReadIsotope<A, C>,
{
    fn get<E: entity::Ref<Archetype = A>>(
        &self,
        entity: E,
        discrim: C::Discrim,
    ) -> system::RefOrDefault<'_, C>
    where
        C: comp::Must<A>,
    {
        self.isotope_accessor.get(entity, discrim)
    }

    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E, discrim: C::Discrim) -> Option<&C> {
        self.isotope_accessor.try_get(entity, discrim)
    }

    type IsotopeRefMap<'t> = impl Iterator<Item = (<C as comp::Isotope<A>>::Discrim, &'t C)> + 't where Self: 't;

    fn get_all<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Self::IsotopeRefMap<'_> {
        self.isotope_accessor.get_all::<E>(entity)
    }
}

impl<A, C, M, S> system::WriteIsotope<A, C> for FullIsotopeAccessor<A, C, M, S>
where
    A: Archetype,
    C: comp::Isotope<A>,
    M: ops::Deref<Target = HashMap<usize, Arc<RwLock<C::Storage>>>>,
    IsotopeAccessor<A, C, S>: system::ReadIsotope<A, C>,
{
    // TODO
}

#[cfg(test)]
static_assertions::assert_impl_all!(Components: Send, Sync);

/// Stores the thread-safe global states in a world.
pub struct SyncGlobals {
    /// Global states that can be concurrently accessed by systems on other threads.
    pub(in crate::world) sync_globals:
        HashMap<DbgTypeId, (referrer::SingleVtable, RwLock<Box<dyn Any + Send + Sync>>)>,
}

impl SyncGlobals {
    /// Creates a dummy, empty global store used for testing.
    pub fn empty() -> Self { Self { sync_globals: HashMap::new() } }

    /// Retrieves a read-only, shared reference to the given global state.
    ///
    /// # Panics
    /// - if the global state is not used in any systems
    /// - if another thread is exclusively accessing the same archetyped component.
    pub fn read<G: Global + Send + Sync>(&self) -> impl ops::Deref<Target = G> + '_ {
        let (_, lock) = match self.sync_globals.get(&TypeId::of::<G>()) {
            Some(lock) => lock,
            None => panic!(
                "The global state {} cannot be used because it is not used in any systems",
                any::type_name::<G>()
            ),
        };
        let guard = match lock.try_read() {
            Some(guard) => guard,
            None => panic!(
                "The global state {} is currently exclusively locked by another system. Maybe \
                 scheduler bug?",
                any::type_name::<G>()
            ),
        };
        RwLockReadGuard::map(guard, |guard| guard.downcast_ref::<G>().expect("TypeId mismatch"))
    }

    /// Retrieves a writable, exclusive reference to the given global state.
    ///
    /// # Panics
    /// - if the global state is not used in any systems
    /// - if another thread is accessing the same archetyped component.
    pub fn write<G: Global + Send + Sync>(
        &self,
    ) -> impl ops::Deref<Target = G> + ops::DerefMut + '_ {
        let (_, lock) = match self.sync_globals.get(&TypeId::of::<G>()) {
            Some(lock) => lock,
            None => panic!(
                "The global state {} cannot be used because it is not used in any systems",
                any::type_name::<G>()
            ),
        };
        let guard = match lock.try_write() {
            Some(guard) => guard,
            None => panic!(
                "The global state {} is currently used exclusively by another system. Maybe \
                 scheduler bug?",
                any::type_name::<G>()
            ),
        };
        RwLockWriteGuard::map(guard, |guard| guard.downcast_mut::<G>().expect("TypeId mismatch"))
    }

    pub(crate) fn get_mut<G: Global + Send + Sync>(&mut self) -> &mut G {
        let (_, lock) = match self.sync_globals.get_mut(&TypeId::of::<G>()) {
            Some(lock) => lock,
            None => panic!(
                "The global state {} cannot be used because it is not used in any systems",
                any::type_name::<G>()
            ),
        };
        lock.get_mut().downcast_mut::<G>().expect("TypeId mismatch")
    }
}

/// Stores the thread-unsafe global states in a world.
pub struct UnsyncGlobals {
    /// Global states that must be accessed on the main thread.
    pub(in crate::world) unsync_globals: HashMap<DbgTypeId, (referrer::SingleVtable, Box<dyn Any>)>,
}

impl UnsyncGlobals {
    /// Creates a dummy, empty global store used for testing.
    pub fn empty() -> Self { Self { unsync_globals: HashMap::new() } }

    /// Gets a reference to the requested global state.
    /// The object must be marked as thread-unsafe during world creation.
    ///
    /// Since the system is run on main thread,
    /// it is expected that a mutable reference to `UnsyncGlobals` is available.
    pub fn get<G: Global>(&mut self) -> &mut G {
        match self.unsync_globals.get_mut(&TypeId::of::<G>()) {
            Some((_vtable, global)) => global.downcast_mut::<G>().expect("TypeId mismatch"),
            None => panic!(
                "The global state {} cannot be used because it is not used in any systems",
                any::type_name::<G>()
            ),
        }
    }
}
