use std::any::{self, TypeId};
use std::collections::HashMap;
use std::ops;
use std::sync::Arc;

use parking_lot::lock_api::ArcRwLockWriteGuard;
use parking_lot::{RawRwLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::storage::Storage;
use super::typed;
use crate::comp::discrim::Map as _;
use crate::comp::Discrim;
use crate::util::DbgTypeId;
use crate::{comp, entity, system, Archetype};

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

        struct Ret<R: ops::DerefMut> {
            storage: R,
        }

        impl<A: Archetype, C: comp::Simple<A>, S: ops::DerefMut<Target = C::Storage>>
            system::ReadSimple<A, C> for Ret<S>
        {
            fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C> {
                self.storage.get(entity.id())
            }
        }
        impl<A: Archetype, C: comp::Simple<A>, S: ops::DerefMut<Target = C::Storage>>
            system::WriteSimple<A, C> for Ret<S>
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

        IsotopeAccessor { storages, on_missing: NoneOnMissingStorage }
    }

    /// Creates a read-only, shared accessor to specific discriminants of the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is exclusively accessing any of the requested discriminants.
    pub fn read_partial_isotope_storage<'t, A: Archetype, C: comp::Isotope<A>>(
        &'t self,
        discrims: &'t [usize],
    ) -> impl system::ReadIsotope<A, C> + 't {
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
                .filter(|(&discrim, _)| discrims.contains(&discrim))
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

        IsotopeAccessor { storages, on_missing: PanicIfNotContainOnMissingStorage(discrims) }
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
                // cloning the arc is necessary to avoid self-referential types.
                let storage: Arc<RwLock<C::Storage>> = Arc::clone(storage);
                let storage = match storage.try_write_arc() {
                    Some(guard) => ArcStorageGuard(guard),
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

        IsotopeAccessor {
            storages:   accessor_storages,
            on_missing: LazyCreateOnMissingStorage(full_map),
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
                        Some(guard) => ArcStorageGuard(guard),
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

        IsotopeAccessor { storages, on_missing: PanicOnMissingStorage }
    }
}

struct IsotopeAccessor<A: Archetype, C: comp::Isotope<A>, S, P> {
    storages:   <C::Discrim as Discrim>::Map<S>,
    on_missing: P,
}

impl<
        A: Archetype,
        C: comp::Isotope<A>,
        S: ops::Deref<Target = C::Storage>,
        P: OnMissingStorage<A, C>,
    > system::ReadIsotope<A, C> for IsotopeAccessor<A, C, S, P>
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
        let storage: &C::Storage = match self.storages.find(discrim.into_usize()) {
            Some(storage) => storage,
            None => {
                self.on_missing.handle(discrim);
                return None;
            }
        };

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

impl<A: Archetype, C: comp::Isotope<A>, S: StorageGuard<C::Storage>, P: OnMissingStorage<A, C>>
    IsotopeAccessor<A, C, S, P>
{
    fn get_storage_mut(&mut self, discrim: C::Discrim) -> Option<&mut C::Storage> {
        // borrow checker bug circumvention
        if self.storages.find(discrim.into_usize()).is_some() {
            Some(self.storages.find_mut(discrim.into_usize()).expect("find() returned Some"))
        } else {
            match self.on_missing.handle_mut(discrim) {
                HandleMutResult::None => None,
                HandleMutResult::Added(storage) => {
                    let storage = S::from_arc(storage);
                    self.storages.extend([(discrim.into_usize(), storage)]);
                    Some(
                        self.storages
                            .find_mut(discrim.into_usize())
                            .expect("OnMissingStorage returned Handle::Retry"),
                    )
                }
            }
        }
    }
}

impl<A: Archetype, C: comp::Isotope<A>, S: StorageGuard<C::Storage>, P: OnMissingStorage<A, C>>
    system::WriteIsotope<A, C> for IsotopeAccessor<A, C, S, P>
{
    fn try_get_mut<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        discrim: C::Discrim,
    ) -> Option<&mut C> {
        let storage = self.get_storage_mut(discrim)?;

        // borrow checker bug circumvention
        if storage.get(entity.id()).is_some() {
            return storage.get_mut(entity.id());
        }

        match C::INIT_STRATEGY {
            comp::IsotopeInitStrategy::None => None,
            comp::IsotopeInitStrategy::Default(default) => {
                storage.set(entity.id(), Some(default()));
                Some(storage.get_mut(entity.id()).expect("entity was just assigned as Some"))
            }
        }
    }

    fn set<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        discrim: C::Discrim,
        value: Option<C>,
    ) -> Option<C> {
        let storage = self.get_storage_mut(discrim)?;

        storage.set(entity.id(), value)
    }
}

pub trait StorageGuard<S>: ops::DerefMut<Target = S> {
    fn from_arc(arc: Arc<RwLock<S>>) -> Self;
}
struct ArcStorageGuard<S>(ArcRwLockWriteGuard<RawRwLock, S>);
impl<S> ops::Deref for ArcStorageGuard<S> {
    type Target = S;
    fn deref(&self) -> &S { &self.0 }
}
impl<S> ops::DerefMut for ArcStorageGuard<S> {
    fn deref_mut(&mut self) -> &mut S { &mut self.0 }
}
impl<S> StorageGuard<S> for ArcStorageGuard<S> {
    fn from_arc(arc: Arc<RwLock<S>>) -> Self {
        Self(arc.try_write_arc().expect("lock was just created"))
    }
}

trait OnMissingStorage<A: Archetype, C: comp::Isotope<A>> {
    fn handle(&self, discrim: C::Discrim);
    fn handle_mut(&mut self, discrim: C::Discrim) -> HandleMutResult<C::Storage>;
}
enum HandleMutResult<S> {
    Added(Arc<RwLock<S>>),
    None,
}

struct PanicOnMissingStorage;
fn panic_on_missing(discrim: usize) -> ! {
    panic!(
        "Cannot access isotope {} because it is not in the list of requested discriminants",
        discrim,
    )
}
impl<A: Archetype, C: comp::Isotope<A>> OnMissingStorage<A, C> for PanicOnMissingStorage {
    fn handle(&self, discrim: C::Discrim) { panic_on_missing(discrim.into_usize()) }
    fn handle_mut(&mut self, discrim: C::Discrim) -> HandleMutResult<C::Storage> {
        panic_on_missing(discrim.into_usize())
    }
}

struct NoneOnMissingStorage;
impl<A: Archetype, C: comp::Isotope<A>> OnMissingStorage<A, C> for NoneOnMissingStorage {
    fn handle(&self, _: C::Discrim) {}
    fn handle_mut(&mut self, _: C::Discrim) -> HandleMutResult<C::Storage> { HandleMutResult::None }
}

struct PanicIfNotContainOnMissingStorage<'t>(&'t [usize]);
impl<'t> PanicIfNotContainOnMissingStorage<'t> {
    fn run(&self, discrim: usize) {
        if !self.0.contains(&discrim) {
            panic!(
                "Cannot access isotope {} because it is not in the list of requested discriminants",
                discrim.into_usize()
            )
        }
    }
}
impl<'t, A: Archetype, C: comp::Isotope<A>> OnMissingStorage<A, C>
    for PanicIfNotContainOnMissingStorage<'t>
{
    fn handle(&self, discrim: C::Discrim) { self.run(discrim.into_usize()) }
    fn handle_mut(&mut self, discrim: C::Discrim) -> HandleMutResult<C::Storage> {
        self.run(discrim.into_usize());
        HandleMutResult::None
    }
}

struct LazyCreateOnMissingStorage<R>(R);
impl<R, A: Archetype, C: comp::Isotope<A>> OnMissingStorage<A, C> for LazyCreateOnMissingStorage<R>
where
    R: ops::DerefMut<Target = HashMap<usize, Arc<RwLock<C::Storage>>>>,
{
    fn handle(&self, _: C::Discrim) {}

    fn handle_mut(&mut self, discrim: C::Discrim) -> HandleMutResult<C::Storage> {
        let arc = Arc::default();
        self.0.insert(discrim.into_usize(), Arc::clone(&arc));
        HandleMutResult::Added(arc)
    }
}

#[cfg(test)]
static_assertions::assert_impl_all!(Components: Send, Sync);
