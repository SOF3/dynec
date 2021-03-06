//! Systems are actions performed every frame to manipulate entities and components.
//!
//! A system can declare mutable or immutable access
//! to archetype-specific components and global states.
//! dynec will schedule systems such that systems have unique access to mutable resources they
//! request.
//! Furthermore, systems can request relative execution order through [`Partition`]s.
//! Systems that use thread-unsafe resources (systems that are not [`Send`])
//! are always executed on the main thread.

use std::any::{self, Any, TypeId};
use std::collections::hash_map::DefaultHasher;
use std::sync::Arc;
use std::{fmt, hash, ops};

use crate::entity::ealloc;
use crate::world::Storage;
use crate::{comp, entity, world, Archetype};

/// Provides access to a simple component in a specific archetype.
pub trait ReadSimple<A: Archetype, C: comp::Simple<A>> {
    /// Returns an immutable reference to the component for the specified entity,
    /// or `None` if the component is not present in the entity.
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C>;

    /// Returns an immutable reference to the component for the specified entity.
    ///
    /// This method is infallible, assuming [`comp::Must`] is only implemented
    /// for components with [`comp::SimplePresence::Required`] presence.
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
    fn get<E: entity::Ref<Archetype = A>>(
        &self,
        entity: E,
        discrim: C::Discrim,
    ) -> RefOrDefault<'_, C>
    where
        C: comp::Must<A>;

    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E, discrim: C::Discrim) -> Option<&C>;

    fn get_all<E: entity::Ref<Archetype = A>>(&self, entity: E) -> IsotopeRefMap<'_, A, C>; // TODO abstract to a trait when GATs are stable
}

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

/// Provides access to an isotope component in a specific archetype.
pub trait WriteIsotope<A: Archetype, C: comp::Isotope<A>> {}

/// Provides immutable access to all isotopes of the same type for an entity.
pub struct IsotopeRefMap<'t, A: Archetype, C: comp::Isotope<A>> {
    #[allow(clippy::type_complexity)]
    pub(crate) storages: <&'t [(usize, StorageRefType<C::Storage>)] as IntoIterator>::IntoIter,
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

/// Provides mutable access to all isotopes of the same type for an entity.
pub struct IsotopeMutMap<'t, A: Archetype, C: comp::Isotope<A>> {
    #[allow(clippy::type_complexity)]
    pub(crate) storages: <&'t mut [(usize, StorageMutType<C::Storage>)] as IntoIterator>::IntoIter,
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

// we won't need this anymore if IsotopeRefMap turns into a trait.
pub(crate) type StorageRefType<T> =
    world::state::OwningMappedRwLockReadGuard<Arc<RwLock<dyn Any + Send + Sync>>, T>;
pub(crate) type StorageMutType<T> =
    world::state::OwningMappedRwLockWriteGuard<Arc<RwLock<dyn Any + Send + Sync>>, T>;

/// A partition is a hashable type constructed by system specifications
/// used to constrain system execution order.
/// Two partition objects are considered equivalent if they have the same type and hash.
///
/// Systems can declare an anterior or posterior dependency on a partition.
/// If multiple systems specify a dependency for an equivalent partition,
/// it is guaranteed that all anterior systems will finish executing
/// before any posterior system starts executing,
/// effectively creating a "partition" between the anterior and posterior systems.
pub trait Partition: sealed::Sealed + Send + Sync + 'static {
    /// Describes the partition as [`fmt::Debug`].
    fn describe(&self, f: &mut fmt::Formatter) -> fmt::Result;

    /// Computes the hash of this component.
    fn compute_hash(&self) -> u64;

    /// Checks whether two parttions are equivalent.
    fn equals(&self, other: &dyn Partition) -> bool;

    /// Converts the object to an `Any`.
    fn as_any(&self) -> &dyn std::any::Any;
}

impl<T: fmt::Debug + Eq + hash::Hash + 'static> sealed::Sealed for T {}

impl<T: fmt::Debug + Eq + hash::Hash + Send + Sync + 'static> Partition for T {
    fn describe(&self, f: &mut fmt::Formatter) -> fmt::Result { writeln!(f, "{:?}", self) }

    fn compute_hash(&self) -> u64 {
        use hash::Hasher;

        let mut hasher = DefaultHasher::new();
        hash::Hash::hash(&TypeId::of::<Self>(), &mut hasher);
        hash::Hash::hash(self, &mut hasher);
        hasher.finish()
    }

    fn equals(&self, other: &dyn Partition) -> bool {
        match other.as_any().downcast_ref::<Self>() {
            Some(other) => self == other,
            None => false,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}

/// A wrapper type for trait objects of [`Partition`]
/// that implements [`Eq`] and [`hash::Hash`] in a type-dependent manner.
pub(crate) struct PartitionWrapper(pub(crate) Box<dyn Partition>);

impl fmt::Debug for PartitionWrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.0.describe(f) }
}

impl PartialEq for PartitionWrapper {
    fn eq(&self, other: &Self) -> bool { (&*self.0).equals(&*other.0) }
}

impl Eq for PartitionWrapper {}

impl hash::Hash for PartitionWrapper {
    fn hash<H: hash::Hasher>(&self, state: &mut H) { self.0.compute_hash().hash(state); }
}

mod sealed {
    pub trait Sealed {}
}

/// A system requests some resources, stores some states of its own and is runnable with the
/// requested resources.
///
/// There may be multiple instances of the same implementor type.
/// This is meaningful as they may have different states.
pub trait Sendable: Send {
    /// Describes this instance of system.
    ///
    /// The method is only called when the system was initially scheduled,
    /// but it should return a consistent value.
    fn get_spec(&self) -> Spec;

    /// Runs the system.
    fn run(
        &mut self,
        globals: &world::SyncGlobals,
        components: &world::Components,
        ealloc_shard_map: &mut ealloc::ShardMap,
    );
}

/// A variant of [`Sendable`] that runs on the main thread only,
/// but allows storing [`Send`] states
/// and accessing non-<code>[Send] + [Sync]</code> global states.
pub trait Unsendable {
    /// Describes this instance of system.
    ///
    /// The method is only called when the system was initially scheduled,
    /// but it should return a consistent value.
    fn get_spec(&self) -> Spec;

    /// Runs the system.
    fn run(
        &mut self,
        sync_globals: &world::SyncGlobals,
        unsync_globals: &mut world::UnsyncGlobals,
        components: &world::Components,
        ealloc_shard_map: &mut ealloc::ShardMap,
    );
}

pub mod spec;
use parking_lot::RwLock;
#[doc(inline)]
pub use spec::Spec;
