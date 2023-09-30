use std::any::{type_name, TypeId};
use std::ops;

use parking_lot::{RwLockReadGuard, RwLockWriteGuard};

use crate::world::{self};
use crate::{comp, system, Archetype};

/// Provides access to a simple component in a specific archetype.
pub type ReadSimple<'t, A: Archetype, C: comp::Simple<A>> = system::AccessSingle<
    A,
    C,
    impl ops::Deref<Target = <C as comp::SimpleOrIsotope<A>>::Storage> + 't,
>;

/// Provides access to a simple component in a specific archetype.
pub type WriteSimple<'t, A: Archetype, C: comp::Simple<A>> = system::AccessSingle<
    A,
    C,
    impl ops::DerefMut<Target = <C as comp::SimpleOrIsotope<A>>::Storage> + 't,
>;

impl world::Components {
    /// Creates a read-only, shared accessor to the given archetyped simple component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems
    /// - if another thread is exclusively accessing the same archetyped component.
    pub fn read_simple_storage<A: Archetype, C: comp::Simple<A>>(&self) -> ReadSimple<A, C> {
        let storage = match self.archetype::<A>().simple_storages.get(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                type_name::<A>(),
                type_name::<C>()
            ),
        };
        let guard = match storage.storage.try_read() {
            Some(guard) => guard,
            None => panic!(
                "The component {}/{} is currently exclusively locked by another system. Maybe \
                 scheduler bug?",
                type_name::<A>(),
                type_name::<C>()
            ),
        };
        let guard = RwLockReadGuard::map(guard, |storage| storage.downcast_ref::<C>());

        system::AccessSingle::new(guard)
    }

    /// Creates a writable, exclusive accessor to the given archetyped simple component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is accessing the same archetyped component.
    pub fn write_simple_storage<A: Archetype, C: comp::Simple<A>>(&self) -> WriteSimple<A, C> {
        let storage = match self.archetype::<A>().simple_storages.get(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                type_name::<A>(),
                type_name::<C>()
            ),
        };
        let guard = match storage.storage.try_write() {
            Some(guard) => guard,
            None => panic!(
                "The component {}/{} is currently used by another system. Maybe scheduler bug?",
                type_name::<A>(),
                type_name::<C>()
            ),
        };
        let guard = RwLockWriteGuard::map(guard, |storage| storage.downcast_mut::<C>());

        system::AccessSingle::new(guard)
    }

    /// Exclusively accesses a simple component type in offline mode.
    ///
    /// Requires a mutable reference to the world to ensure that the world is offline.
    pub fn get_simple_storage<A: Archetype, C: comp::Simple<A>>(
        &mut self,
    ) -> system::AccessSingle<A, C, &mut C::Storage> {
        let typed = self.archetype_mut::<A>();
        let storage = match typed.simple_storages.get_mut(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {} cannot be retrieved because it is not used in any systems",
                type_name::<C>()
            ),
        };
        let storage = storage.get_storage::<C>();
        system::AccessSingle::new(storage)
    }
}
