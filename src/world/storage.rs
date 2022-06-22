//! A storage is the data structure where components of the same type for all entities are stored.

use std::any::{self, Any};
use std::sync::Arc;

use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

use crate::{comp, entity, Archetype};

mod vec;
pub use vec::VecStorage as Vec;

mod tree;
pub use tree::Tree;

pub mod mux;
pub use mux::Mux;

/// A [`Mux`] that uses a [`Tree`] and [`Vec`] as the backends.
pub type MapVecMux<E, C> = Mux<E, C, Tree<E, C>, Vec<E, C>>;

/// A storage for storing component data.
pub trait Storage: Default + Send + Sync + 'static {
    type RawEntity: entity::Raw;
    type Comp;

    /// Gets a shared reference to the component for a specific entity if it is present.
    fn get(&self, id: Self::RawEntity) -> Option<&Self::Comp>;

    /// Gets a mutable reference to the component for a specific entity if it is present.
    fn get_mut(&mut self, id: Self::RawEntity) -> Option<&mut Self::Comp>;

    /// Sets or removes the component for a specific entity,
    /// returning the original value if it was present.
    fn set(&mut self, id: Self::RawEntity, value: Option<Self::Comp>) -> Option<Self::Comp>;

    /// Returns an immutable iterator over the storage, ordered by entity index order.
    fn iter(&self) -> Box<dyn Iterator<Item = (Self::RawEntity, &Self::Comp)> + '_>;

    /// Returns a mutable iterator over the storage, ordered by entity index order.
    fn iter_mut(&mut self) -> Box<dyn Iterator<Item = (Self::RawEntity, &mut Self::Comp)> + '_>;
}

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
    pub(crate) fn read_storage<C: comp::Simple<A>>(&self) -> MappedRwLockReadGuard<'_, C::Storage> {
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
    pub(crate) fn write_storage<C: comp::Simple<A>>(
        &self,
    ) -> MappedRwLockWriteGuard<'_, C::Storage> {
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

// TODO: isotope components

pub(crate) struct Isotope<A: Archetype> {
    init_strategy: comp::IsotopeInitStrategy<A>, // TODO
}

impl<A: Archetype> Isotope<A> {
    pub(crate) fn new<C: comp::Isotope<A>>() -> Self { todo!() }
}

pub(crate) struct IsotopeFactory<A: Archetype> {
    builder: fn() -> Isotope<A>, // TODO
}
