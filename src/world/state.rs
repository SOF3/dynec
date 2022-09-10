use std::any::{self, Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops;
use std::sync::Arc;

use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

use super::storage::Storage;
use super::typed::{self, PaddedIsotopeIdentifier};
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
    /// - if the archetyped component is not used in any systems
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

    /// Creates a read-only, shared accessor to the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems
    /// - if another thread is exclusively accessing the same archetyped component.
    pub fn read_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
        &self,
        discrims: Option<&[usize]>,
    ) -> impl system::ReadIsotope<A, C> + '_ {
        let storages: Vec<_> = {
            let storages = self.archetype::<A>().isotope_storages.read();
            storages
                .range(PaddedIsotopeIdentifier::range::<C>())
                .filter_map(|(ty, storage)| {
                    Some((
                        {
                            let discrim = ty.expect_discrim();
                            if let Some(discrims) = discrims {
                                if !discrims.contains(&discrim) {
                                    return None;
                                }
                            }
                            discrim
                        },
                        storage,
                    ))
                })
                .map(|(discrim, storage)| {
                    (discrim, {
                        let storage = Arc::clone(&storage.storage);
                        OwningMappedRwLockReadGuard::new(storage, |storage| {
                            let guard = match storage.try_read() {
                                Some(guard) => guard,
                                None => panic!(
                                    "The component {}/{}/{} is currently used by another system. \
                                     Maybe scheduler bug?",
                                    any::type_name::<A>(),
                                    any::type_name::<C>(),
                                    discrim,
                                ),
                            };
                            RwLockReadGuard::map(guard, |storage| storage.downcast_ref::<C>())
                        })
                    })
                })
                .collect()
        };

        ReadIsotopeStorage {
            storages,
            default: match C::INIT_STRATEGY {
                comp::IsotopeInitStrategy::None => None,
                comp::IsotopeInitStrategy::Default(factory) => Some(factory),
            },
        }
    }

    /// Creates a writable, exclusive accessor to the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems
    /// - if another thread is accessing the same archetyped component.
    pub fn write_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
        &self,
        _discrim: Option<&[usize]>,
    ) -> impl system::WriteIsotope<A, C> + '_ {
        WriteIsotopeStorage(PhantomData)
    }
}

struct ReadIsotopeStorage<C, S: ops::Deref> {
    // TODO customize the implementation of discriminant lookup to allow O(1) indexing in
    // small universe of concourse
    storages: Vec<(usize, S)>,
    default:  Option<fn() -> C>,
}

impl<A: Archetype, C: comp::Isotope<A>, S: ops::Deref<Target = C::Storage>>
    system::ReadIsotope<A, C> for ReadIsotopeStorage<C, S>
{
    type IsotopeRefMap<'t> = impl Iterator<Item = (<C as comp::Isotope<A>>::Discrim, &'t C)> + 't where S: 't;

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
            None => system::RefOrDefault(system::BorrowedOwned::Owned(self
                .default
                .expect("C: comp::Must<A>")(
            ))),
        }
    }

    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E, discrim: C::Discrim) -> Option<&C> {
        let discrim = discrim.into_usize();

        // if storage does not exist, the component does not exist yet.
        let (_, storage) = self.storages.iter().find(|(key, _)| *key == discrim)?;

        storage.get(entity.id())
    }

    fn get_all<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Self::IsotopeRefMap<'_> {
        /// Provides immutable access to all isotopes of the same type for an entity.
        struct Ret<'t, A: Archetype, C: comp::Isotope<A>, S: ops::Deref<Target = C::Storage>> {
            #[allow(clippy::type_complexity)]
            storages: <&'t [(usize, S)] as IntoIterator>::IntoIter,
            index:    A::RawEntity,
            _ph:      PhantomData<fn() -> C>,
        }

        impl<'t, A: Archetype, C: comp::Isotope<A>, S: ops::Deref<Target = C::Storage>> Iterator
            for Ret<'t, A, C, S>
        {
            type Item = (C::Discrim, &'t C);

            fn next(&mut self) -> Option<Self::Item> {
                for (discrim, storage) in self.storages.by_ref() {
                    let discrim = <C::Discrim as comp::Discrim>::from_usize(*discrim);
                    let value = match storage.get(self.index) {
                        Some(value) => value,
                        None => continue,
                    };

                    return Some((discrim, value));
                }

                None
            }
        }

        Ret { storages: self.storages.iter(), index: entity.id(), _ph: PhantomData }
    }
}

struct WriteIsotopeStorage<A: Archetype, C: comp::Isotope<A>>(PhantomData<(A, C)>);

impl<A: Archetype, C: comp::Isotope<A>> system::ReadIsotope<A, C> for WriteIsotopeStorage<A, C> {
    type IsotopeRefMap<'t> = impl Iterator<Item = (<C as comp::Isotope<A>>::Discrim, &'t C)> + 't;

    fn get<E: entity::Ref<Archetype = A>>(
        &self,
        _entity: E,
        _discrim: C::Discrim,
    ) -> system::RefOrDefault<'_, C>
    where
        C: comp::Must<A>,
    {
        todo!()
    }

    fn try_get<E: entity::Ref<Archetype = A>>(
        &self,
        _entity: E,
        _discrim: C::Discrim,
    ) -> Option<&C> {
        todo!()
    }

    fn get_all<E: entity::Ref<Archetype = A>>(&self, _entity: E) -> Self::IsotopeRefMap<'_> {
        if true {
            todo!()
        }

        std::iter::empty()
    }
}
impl<A: Archetype, C: comp::Isotope<A>> system::WriteIsotope<A, C> for WriteIsotopeStorage<A, C> {}

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

#[ouroboros::self_referencing]
pub(crate) struct OwningMappedRwLockReadGuard<L: 'static, U: 'static> {
    lock:  L,
    #[borrows(lock)]
    #[covariant]
    guard: MappedRwLockReadGuard<'this, U>,
}

impl<L: 'static, U: 'static> ops::Deref for OwningMappedRwLockReadGuard<L, U> {
    type Target = U;

    fn deref(&self) -> &Self::Target { self.borrow_guard() }
}

#[ouroboros::self_referencing]
pub(crate) struct OwningMappedRwLockWriteGuard<L: 'static, U: 'static> {
    lock:  L,
    #[borrows(lock)]
    #[covariant]
    guard: MappedRwLockWriteGuard<'this, U>,
}

impl<L: 'static, U: 'static> OwningMappedRwLockWriteGuard<L, U> {
    /// Gets `U` mutably.
    ///
    /// # Safety
    /// TODO idk...
    pub(crate) unsafe fn borrow_guard_mut(&mut self) -> &mut U {
        &mut *self.with_guard_mut(|guard| &mut **guard as *mut U)
    }
}
