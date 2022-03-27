//! Systems are actions performed every frame to manipulate entities and components.
//!
//! A system can declare mutable or immutable access
//! to archetype-specific components and global states.
//! dynec will schedule systems such that systems have unique access to mutable resources they
//! request.
//! Furthermore, systems can request relative execution order through [`Partition`]s.
//! Systems that use thread-unsafe resources (systems that are not [`Send`])
//! are always executed on the main thread.

use std::any::TypeId;
use std::collections::hash_map::DefaultHasher;
use std::hash;
use std::marker::PhantomData;

use crate::{component, util, Archetype, Global};

/// Provides immutable or mutable access to a simple component in a specific archetype.
pub struct Simple<A: Archetype, R: util::Ref>
where
    R::Target: component::Simple<A>,
{
    _ph: PhantomData<(A, R)>,
}

/// Provides immutable or mutable access to an isotope component in a specific archetype.
pub struct Isotope<A: Archetype, R: util::Ref>
where
    R::Target: component::Isotope<A>,
{
    _ph: PhantomData<(A, R)>,
}

/// Provides immutable or mutable access to a global state.
pub struct Glob<R: util::Ref>
where
    R::Target: Global,
{
    _ph: PhantomData<R>,
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
pub trait Partition: sealed::Sealed + 'static {
    fn compute_hash(&self) -> u64;

    fn equals(&self, other: &dyn Partition) -> bool;

    fn as_any(&self) -> &dyn std::any::Any;
}

impl<T: Eq + hash::Hash + 'static> sealed::Sealed for T {}

impl<T: Eq + hash::Hash + 'static> Partition for T {
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

mod sealed {
    pub trait Sealed {}
}

pub mod spec;
#[doc(inline)]
pub use spec::Spec;
