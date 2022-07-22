use std::any::{self, Any};
#[cfg(test)]
use std::ops;
use std::sync::Arc;

use parking_lot::RwLock;

use super::Storage;
use crate::{comp, Archetype};

pub(crate) struct Simple<A: Archetype> {
    /// The init strategy of the component.
    pub(crate) init_strategy:    comp::SimpleInitStrategy<A>,
    /// The actual storage object. Downcasts to `C::Storage`.
    pub(crate) storage:          Arc<RwLock<dyn Any + Send + Sync>>,
    /// This is a function pointer to [`fn@fill_init_simple`] with the correct type parameters.
    pub(crate) fill_init_simple: fn(&mut dyn Any, A::RawEntity, &mut comp::Map<A>),
}

impl<A: Archetype> Simple<A> {
    pub(crate) fn new<C: comp::Simple<A>>() -> Self {
        Self {
            init_strategy:    C::INIT_STRATEGY,
            storage:          Arc::new(RwLock::new(C::Storage::default()))
                as Arc<RwLock<dyn Any + Send + Sync>>,
            fill_init_simple: fill_init_simple::<A, C>,
        }
    }

    /// Acquires a shared lock on the storage in online mode.
    #[cfg(test)]
    pub(crate) fn read_storage<C: comp::Simple<A>>(
        &self,
    ) -> impl ops::Deref<Target = C::Storage> + '_ {
        use parking_lot::RwLockReadGuard;

        match self.storage.try_read() {
            Some(storage) => match RwLockReadGuard::try_map(storage, |storage| {
                storage.downcast_ref::<C::Storage>()
            }) {
                Ok(storage) => storage,
                Err(_) => panic!("TypeId mismatch"),
            },
            None => panic!(
                "Storage for `{}`/`{}` is locked exclusively. Maybe scheduler bug?",
                any::type_name::<A>(),
                any::type_name::<C>(),
            ),
        }
    }

    /// Acquires an exclusive lock on the storage in online mode.
    #[cfg(test)]
    pub(crate) fn write_storage<C: comp::Simple<A>>(
        &self,
    ) -> impl ops::Deref<Target = C::Storage> + ops::DerefMut + '_ {
        use parking_lot::RwLockWriteGuard;
        match self.storage.try_write() {
            Some(storage) => match RwLockWriteGuard::try_map(storage, |storage| {
                storage.downcast_mut::<C::Storage>()
            }) {
                Ok(storage) => storage,
                Err(_) => panic!("TypeId mismatch"),
            },
            None => panic!(
                "Storage for `{}`/`{}` is already locked. Maybe scheduler bug?",
                any::type_name::<A>(),
                any::type_name::<C>(),
            ),
        }
    }

    /// Gets the inner storage in offline mode.
    pub(crate) fn get_storage<C: comp::Simple<A>>(&mut self) -> &mut C::Storage {
        let storage = Arc::get_mut(&mut self.storage)
            .expect("Storage Arc clones should not outlive system execution")
            .get_mut();
        storage.downcast_mut::<C::Storage>().expect("TypeId mismatch")
    }
}

fn fill_init_simple<A: Archetype, C: comp::Simple<A>>(
    storage: &mut dyn Any,
    entity: A::RawEntity,
    components: &mut comp::Map<A>,
) {
    let storage: &mut C::Storage = storage.downcast_mut().expect("function pointer mismatch");

    if let Some(comp) = components.remove_simple::<C>() {
        storage.set(entity, Some(comp));
    } else if let comp::SimplePresence::Required = C::PRESENCE {
        panic!(
            "Cannot create an entity of type `{}` without explicitly passing a component of type \
             `{}`",
            any::type_name::<A>(),
            any::type_name::<C>(),
        );
    }
}
