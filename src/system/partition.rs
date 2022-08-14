//! Partitions enforce execution order of systems.
//! See [`Partition`] documentation for more.

use core::fmt;
use std::any::TypeId;
use std::collections::hash_map::DefaultHasher;
use std::hash;

use crate::util::DbgTypeId;
use crate::Archetype;

mod sealed {
    pub trait Sealed {}
}

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
pub struct Wrapper(
    /// The partition value as a trait object
    pub Box<dyn Partition>,
);

impl fmt::Debug for Wrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.0.describe(f) }
}

impl PartialEq for Wrapper {
    fn eq(&self, other: &Self) -> bool { (&*self.0).equals(&*other.0) }
}

impl Eq for Wrapper {}

impl hash::Hash for Wrapper {
    fn hash<H: hash::Hasher>(&self, state: &mut H) { self.0.compute_hash().hash(state); }
}

/// Builtin partition for partitioning component accessors before entity creators.
///
/// # Entity creators
/// All systems that request an [`EntityCreator<A>`](crate::system::EntityCreator)
/// run after `EntityCreationPartition::new::<A>()`.
/// This is to ensure systems that access strong references of this archetype
/// are not run between the execution of this system and the cycle join,
/// during which the entity is in uninitialized state.
///
/// If the author of the entity-creating system can ensure that
/// no unknown systems can access the uninitialized reference it produces
/// (e.g. if it does not store the created entity anywhere,
/// or if the created entity is only accessiblee in a private component
/// before the current cycle ends),
/// on the argument that requested a [`EntityCreator<A>`](crate::system::EntityCreator),
/// apply the attribute `#[dynec(entity_creator(no_partition))]`;
/// for manually defined systems that do not use the macro,
/// call [`EntityCreatorRequest::no_partition`](crate::system::spec::EntityCreatorRequest).
/// In such case, the system is responsible for hiding the entity references it creates
/// from other systems that assume strong references are valid.
///
/// # Component accessors
/// By default, all systems that request read/write access
/// to components/globals that own strong references to an archetype `A`
/// run before `EntityCreationPartition::new::<A>()`.
///
/// If the system does not assume on the validity of entity reference
/// (i.e. always accessing its components through the `try_*` APIs),
/// this dependency can be safely removed by applying the attribute
/// `#[dynec(arg_type(maybe_uninit(A)))]` on the arguments that request the component,
/// where `arg_type` is `simple`/`isotope`/`global` depending on the argument type.
/// Note that `A` is the archetype of the referenced entity, not of the component.
/// For manually defined systems that do not use the macro,
/// call [`SimpleRequest::maybe_uninit::<A>()`](crate::system::spec::SimpleRequest) or equivalent.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct EntityCreationPartition {
    pub(crate) ty: DbgTypeId,
}

impl EntityCreationPartition {
    /// Constructs an EntityCreationPartition with the given archetype.
    pub fn new<A: Archetype>() -> Self { Self { ty: DbgTypeId::of::<A>() } }
}

#[cfg(test)]
crate::assert_partition!(EntityCreationPartition);
