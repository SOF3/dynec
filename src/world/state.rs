use std::any::{self, Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops;

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::storage::Storage;
use super::typed;
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
        let storage_lock = match self.archetype::<A>().simple_storages.get(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };
        let guard = match storage_lock.try_read() {
            Some(guard) => guard,
            None => panic!(
                "The component {}/{} is currently exclusively locked by another system. Maybe \
                 scheduler bug?",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };
        let guard = RwLockReadGuard::map(guard, |storage| {
            storage.as_any().downcast_ref::<Storage<A, C>>().expect("TypeId mismatch")
        });

        struct Ret<A: Archetype, C: comp::Simple<A>, S: ops::Deref<Target = Storage<A, C>>> {
            storage: S,
        }

        impl<A: Archetype, C: comp::Simple<A>, S: ops::Deref<Target = Storage<A, C>>>
            system::ReadSimple<A, C> for Ret<A, C, S>
        {
            fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C> {
                self.storage.get(entity.id().0)
            }
        }

        Ret { storage: guard }
    }

    /// Creates a writable, exclusive accessor to the given archetyped simple component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems
    /// - if another thread is accessing the same archetyped component.
    pub fn write_simple_storage<A: Archetype, C: comp::Simple<A>>(
        &self,
    ) -> impl system::WriteSimple<A, C> + '_ {
        let storage_lock = match self.archetype::<A>().simple_storages.get(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };
        let guard = match storage_lock.try_write() {
            Some(guard) => guard,
            None => panic!(
                "The component {}/{} is currently used by another system. Maybe scheduler bug?",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };
        let guard = RwLockWriteGuard::map(guard, |storage| {
            storage.as_any_mut().downcast_mut::<Storage<A, C>>().expect("TypeId mismatch")
        });

        struct Ret<
            A: Archetype,
            C: comp::Simple<A>,
            S: ops::Deref<Target = Storage<A, C>> + ops::DerefMut,
        > {
            storage: S,
        }

        impl<
                A: Archetype,
                C: comp::Simple<A>,
                S: ops::Deref<Target = Storage<A, C>> + ops::DerefMut,
            > system::ReadSimple<A, C> for Ret<A, C, S>
        {
            fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C> {
                self.storage.get(entity.id().0)
            }
        }
        impl<
                A: Archetype,
                C: comp::Simple<A>,
                S: ops::Deref<Target = Storage<A, C>> + ops::DerefMut,
            > system::WriteSimple<A, C> for Ret<A, C, S>
        {
            fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C> {
                self.storage.get_mut(entity.id().0)
            }

            fn set<E: entity::Ref<Archetype = A>>(
                &mut self,
                entity: E,
                value: Option<C>,
            ) -> Option<C> {
                self.storage.set(entity.id().0, value)
            }
        }

        Ret { storage: guard }
    }

    /// Creates a read-only, shared accessor to the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems
    /// - if another thread is exclusively accessing the same archetyped component.
    pub fn read_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
        &self,
        discrim: Option<&[usize]>,
    ) -> impl system::ReadIsotope<A, C> + '_ {
        struct Ret<A: Archetype, C: comp::Isotope<A>>(PhantomData<(A, C)>);

        impl<A: Archetype, C: comp::Isotope<A>> system::ReadIsotope<A, C> for Ret<A, C> {}
        impl<A: Archetype, C: comp::Isotope<A>> system::WriteIsotope<A, C> for Ret<A, C> {}

        Ret(PhantomData)
    }

    /// Creates a writable, exclusive accessor to the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems
    /// - if another thread is accessing the same archetyped component.
    pub fn write_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
        &self,
        discrim: Option<&[usize]>,
    ) -> impl system::WriteIsotope<A, C> + '_ {
        struct Ret<A: Archetype, C: comp::Isotope<A>>(PhantomData<(A, C)>);

        impl<A: Archetype, C: comp::Isotope<A>> system::ReadIsotope<A, C> for Ret<A, C> {}
        impl<A: Archetype, C: comp::Isotope<A>> system::WriteIsotope<A, C> for Ret<A, C> {}

        Ret(PhantomData)
    }
}

#[cfg(test)]
static_assertions::assert_impl_all!(Components: Send, Sync);

/// Stores the thread-safe global states in a world.
pub struct SyncGlobals {
    /// Global states that can be concurrently accessed by systems on other threads.
    pub(in crate::world) sync_globals: HashMap<DbgTypeId, RwLock<Box<dyn Any + Send + Sync>>>,
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
        let lock = match self.sync_globals.get(&TypeId::of::<G>()) {
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
        let lock = match self.sync_globals.get(&TypeId::of::<G>()) {
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
}

/// Stores the thread-unsafe global states in a world.
pub struct UnsyncGlobals {
    /// Global states that must be accessed on the main thread.
    pub(in crate::world) unsync_globals: HashMap<DbgTypeId, Box<dyn Any>>,
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
            Some(global) => global.downcast_mut::<G>().expect("TypeId mismatch"),
            None => panic!(
                "The global state {} cannot be used because it is not used in any systems",
                any::type_name::<G>()
            ),
        }
    }
}
