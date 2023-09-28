use std::any::{self, Any};
#[cfg(test)]
use std::ops;
use std::sync::Arc;

use parking_lot::RwLock;

use super::Storage;
use crate::comp::any::DepGetter;
use crate::entity::referrer;
use crate::{comp, Archetype};

/// Constructor for [`Simple`].
pub(crate) fn builder<A: Archetype, C: comp::Simple<A>>() -> Box<dyn Any> {
    Box::new(Simple::<A>::new::<C>())
}

/// Storage and metadata for a simple component.
pub(crate) struct Simple<A: Archetype> {
    /// The init strategy of the component.
    pub(crate) dep_list: comp::DepList,
    /// The actual storage object. Downcasts to `C::Storage`.
    pub(crate) storage:  Arc<RwLock<dyn AnySimpleStorage<A>>>,
}

impl<A: Archetype> Simple<A> {
    pub(crate) fn new<C: comp::Simple<A>>() -> Self {
        Self {
            dep_list: C::INIT_STRATEGY.checked_deps(),
            storage:  Arc::new(RwLock::new(SimpleStorage::<A, C>(C::Storage::default())))
                as Arc<RwLock<dyn AnySimpleStorage<A>>>,
        }
    }

    /// Acquires a shared lock on the storage in online mode.
    #[cfg(test)]
    pub(crate) fn read_storage<C: comp::Simple<A>>(
        &self,
    ) -> impl ops::Deref<Target = C::Storage> + '_ {
        use parking_lot::RwLockReadGuard;

        match self.storage.try_read() {
            Some(storage) => RwLockReadGuard::map(storage, |storage| storage.downcast_ref::<C>()),
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
    ) -> impl ops::DerefMut<Target = C::Storage> + '_ {
        use parking_lot::RwLockWriteGuard;
        match self.storage.try_write() {
            Some(storage) => RwLockWriteGuard::map(storage, |storage| storage.downcast_mut::<C>()),
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
        storage.downcast_mut::<C>()
    }
}

pub(crate) trait AnySimpleStorage<A: Archetype>: Send + Sync {
    fn as_any(&self) -> &(dyn Any + Send + Sync);

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);

    /// Fills a simple component of with the initial value.
    fn fill_init_simple(
        &mut self,
        entity: A::RawEntity,
        comp_map: &mut comp::Map<A>,
        dep_getter: DepGetter<'_, A>,
    );

    /// Returns true if [`C::IS_FINALIZER`](comp::Simple::IS_FINALIZER).
    /// and the component exists for the given entity.
    fn has_finalizer(&self, entity: A::RawEntity) -> bool;

    /// Clears the component data for an entity if any.
    fn clear_entry(&mut self, entity: A::RawEntity);

    /// Returns a [`referrer::Object`] implementation that visits all components in this storage.
    fn referrer_dyn<'t>(&'t mut self) -> Box<dyn referrer::Object + 't>;

    /// Returns the value for an entity and return it as an [`Any`].
    ///
    /// Due to the poor performance of [`Any`],
    /// this method should not be used unless component type elision is necessary.
    fn get_any(&self, entity: A::RawEntity) -> Option<&dyn Any>;
}

impl<A: Archetype> dyn AnySimpleStorage<A> {
    pub(crate) fn downcast_ref<C: comp::Simple<A>>(&self) -> &C::Storage {
        &self.as_any().downcast_ref::<SimpleStorage<A, C>>().expect("TypeId mismatch").0
    }

    pub(crate) fn downcast_mut<C: comp::Simple<A>>(&mut self) -> &mut C::Storage {
        &mut self.as_any_mut().downcast_mut::<SimpleStorage<A, C>>().expect("TypeId mismatch").0
    }
}

struct SimpleStorage<A: Archetype, C: comp::Simple<A>>(C::Storage);

impl<A: Archetype, C: comp::Simple<A>> AnySimpleStorage<A> for SimpleStorage<A, C> {
    fn as_any(&self) -> &(dyn Any + Send + Sync) { self }

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) { self }

    fn fill_init_simple(
        &mut self,
        entity: A::RawEntity,
        comp_map: &mut comp::Map<A>,
        dep_getter: DepGetter<'_, A>,
    ) {
        if let Some(comp) = comp_map.remove_simple::<C>() {
            self.0.set(entity, Some(comp));
        } else if let comp::InitStrategy::Auto(initer) = C::INIT_STRATEGY {
            self.0.set(entity, Some(initer.f.init(dep_getter)));
        } else if let comp::Presence::Required = C::PRESENCE {
            panic!(
                "Cannot create an entity of type `{}` without explicitly passing a component of \
                 type `{}`",
                any::type_name::<A>(),
                any::type_name::<C>(),
            );
        }
    }

    fn has_finalizer(&self, entity: A::RawEntity) -> bool {
        if !C::IS_FINALIZER {
            return false;
        }

        self.0.get(entity).is_some()
    }

    fn clear_entry(&mut self, entity: A::RawEntity) { self.0.set(entity, None); }

    fn referrer_dyn<'t>(&'t mut self) -> Box<dyn referrer::Object + 't> {
        Box::new(referrer::UnnamedIter(self.0.iter_chunks_mut().flat_map(|chunk| chunk.slice)))
    }

    fn get_any(&self, entity: A::RawEntity) -> Option<&dyn Any> {
        self.0.get(entity).map(|v| v as &dyn Any)
    }
}
