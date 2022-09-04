use std::sync::Arc;
use std::{any, ops};

use parking_lot::RwLock;

use crate::storage::AnyIsotopeStorage;
use crate::world::Storage;
use crate::{comp, entity, world, Archetype};

/// Provides access to a simple component in a specific archetype.
pub trait ReadSimple<A: Archetype, C: comp::Simple<A>> {
    /// Returns an immutable reference to the component for the specified entity,
    /// or `None` if the component is not present in the entity.
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C>;

    /// Returns an immutable reference to the component for the specified entity.
    ///
    /// # Panics
    /// This method panics if the entity is not fully initialized yet.
    /// This happens when an entity is newly created and the cycle hasn't joined yet.
    fn get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> &C
    where
        C: comp::Must<A>,
    {
        match self.try_get(entity) {
            Some(comp) => comp,
            None => panic!(
                "Component {}/{} implements comp::Must but is not present",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        }
    }
}

/// Provides access to a simple component in a specific archetype.
pub trait WriteSimple<A: Archetype, C: comp::Simple<A>>: ReadSimple<A, C> {
    /// Returns a mutable reference to the component for the specified entity,
    /// or `None` if the component is not present in the entity.
    ///
    /// Note that this method returns `Option<&mut C>`, not `&mut Option<C>`.
    /// This means setting the Option itself to `Some`/`None` will not modify any stored value.
    /// Use [`WriteSimple::set`] to add/remove a component.
    fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C>;

    /// Returns an immutable reference to the component for the specified entity.
    ///
    /// This method is infallible, assuming [`comp::Must`] is only implemented
    /// for components with [`comp::SimplePresence::Required`] presence.
    fn get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> &mut C
    where
        C: comp::Must<A>,
    {
        match self.try_get_mut(entity) {
            Some(comp) => comp,
            None => panic!(
                "Component {}/{} implements comp::Must but is not present",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        }
    }

    /// Overwrites the component for the specified entity.
    ///
    /// Passing `None` to this method removes the component from the entity.
    /// This leads to a panic for components with [`comp::SimplePresence::Required`] presence.
    fn set<E: entity::Ref<Archetype = A>>(&mut self, entity: E, value: Option<C>) -> Option<C>;
}

/// Provides access to an isotope component in a specific archetype.
pub trait ReadIsotope<A: Archetype, C: comp::Isotope<A>> {
    /// Retrieves the component for the given entity and discriminant.
    ///
    /// This method is infallible for correctly implemented `comp::Must`,
    /// which returns the auto-initialized value for missing components.
    fn get<E: entity::Ref<Archetype = A>>(
        &self,
        entity: E,
        discrim: C::Discrim,
    ) -> RefOrDefault<'_, C>
    where
        C: comp::Must<A>;

    /// Returns an immutable reference to the component for the specified entity and ,
    /// or `None` if the component is not present in the entity.
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E, discrim: C::Discrim) -> Option<&C>;

    /// Iterates over all isotopes of the component type for the given entity.
    fn get_all<E: entity::Ref<Archetype = A>>(&self, entity: E) -> IsotopeRefMap<'_, A, C>; // TODO abstract to a trait when GATs are stable

    /// Creates an accessor with fixed discriminant.
    fn with(&self, discrim: C::Discrim) -> FixedIsotope<&'_ Self, A, C> {
        FixedIsotope { discrim, accessor: self }
    }
}

/// A lazy accessor that may return an owned default value.
pub struct RefOrDefault<'t, C>(pub(crate) BorrowedOwned<'t, C>);

pub(crate) enum BorrowedOwned<'t, C> {
    Borrowed(&'t C),
    Owned(C),
}

impl<'t, C> ops::Deref for RefOrDefault<'t, C> {
    type Target = C;

    fn deref(&self) -> &C {
        match self.0 {
            BorrowedOwned::Borrowed(ref_) => ref_,
            BorrowedOwned::Owned(ref owned) => owned,
        }
    }
}

/// Provides immutable access to all isotopes of the same type for an entity.
pub struct IsotopeRefMap<'t, A: Archetype, C: comp::Isotope<A>> {
    #[allow(clippy::type_complexity)]
    pub(crate) storages: <&'t [(usize, StorageRefType<A, C::Storage>)] as IntoIterator>::IntoIter,
    pub(crate) index:    A::RawEntity,
}

impl<'t, A: Archetype, C: comp::Isotope<A>> Iterator for IsotopeRefMap<'t, A, C> {
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

/// Provides access to an isotope component in a specific archetype.
pub trait WriteIsotope<A: Archetype, C: comp::Isotope<A>> {
    /// Creates an accessor with fixed discriminant.
    fn with(&self, discrim: C::Discrim) -> FixedIsotope<&'_ Self, A, C> {
        FixedIsotope { discrim, accessor: self }
    }
}

/// Provides mutable access to all isotopes of the same type for an entity.
pub struct IsotopeMutMap<'t, A: Archetype, C: comp::Isotope<A>> {
    #[allow(clippy::type_complexity)]
    pub(crate) storages:
        <&'t mut [(usize, StorageMutType<A, C::Storage>)] as IntoIterator>::IntoIter,
    pub(crate) index:    A::RawEntity,
}

impl<'t, A: Archetype, C: comp::Isotope<A>> Iterator for IsotopeMutMap<'t, A, C> {
    type Item = (C::Discrim, &'t mut C);

    fn next(&mut self) -> Option<Self::Item> {
        for (discrim, storage) in self.storages.by_ref() {
            let discrim = <C::Discrim as comp::Discrim>::from_usize(*discrim);

            // safety: TODO idk...
            let value = unsafe { storage.borrow_guard_mut().get_mut(self.index) };
            let value = match value {
                Some(value) => value,
                None => continue,
            };

            return Some((discrim, value));
        }

        None
    }
}

/// An isotope component accessor that only uses a specific isotope known at runtime.
pub struct FixedIsotope<X, A: Archetype, C: comp::Isotope<A>> {
    discrim:  C::Discrim,
    accessor: X,
}

// we won't need this anymore if IsotopeRefMap turns into a trait.
pub(crate) type StorageRefType<A, T> =
    world::state::OwningMappedRwLockReadGuard<Arc<RwLock<dyn AnyIsotopeStorage<A>>>, T>;
pub(crate) type StorageMutType<A, T> =
    world::state::OwningMappedRwLockWriteGuard<Arc<RwLock<dyn AnyIsotopeStorage<A>>>, T>;
