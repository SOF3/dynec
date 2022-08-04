use core::fmt;
use std::any::TypeId;
use std::collections::hash_map::DefaultHasher;
use std::hash;

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
