//! Iterate over entities of an archetype.
//!
//! While individual accessors also provide functions like
//! [`AccessSingle::iter`](access::Single::iter),
//! functions in [`EntityIterator`] use the entity indices
//! from the entity allocator snapshot directly,
//! enabling better performance with chunk partitioning.

use std::marker::PhantomData;
use std::{any, mem, ops};

use super::access::single;
use crate::entity::{ealloc, Raw as _};
use crate::system::access;
use crate::{comp, entity, storage, util, Archetype, Storage};

/// Allows iterating all entities of an archetype.
pub struct EntityIterator<A: Archetype> {
    ealloc: ealloc::Snapshot<A::RawEntity>,
}

impl<A: Archetype> EntityIterator<A> {
    /// Constructs an instance of [`EntityIterator`] that reads from the given allocator.
    ///
    /// Although this function accepts an allocator shard,
    /// it actually reads the global buffer shared between shards,
    /// which is independent of the changes in the current shard.
    /// Hence, the iterator describe the state after the previous tick completes,
    /// which does not include newly initialized entities
    /// and includes those queued for deletion.
    /// This behavior is reasonable, because newly initialized entities should not be accessed at all,
    /// and those queued for deletion may have a finalizer or
    /// be given a finalizer when running later systems,
    /// so those queued for deletion are still included.
    ///
    /// This function is typically called from the code generated by
    /// [`#[system]`](macro@crate::system).
    pub fn new(ealloc: ealloc::Snapshot<A::RawEntity>) -> Self { Self { ealloc } }

    /// Iterates over all entity IDs in this archetype.
    pub fn entities(&self) -> impl Iterator<Item = entity::TempRef<A>> {
        self.ealloc
            .iter_allocated_chunks()
            .flat_map(<A::RawEntity as entity::Raw>::range)
            .map(entity::TempRef::new)
    }

    /// Iterates over all contiguous chunks of entity IDs.
    pub fn chunks(&self) -> impl Iterator<Item = entity::TempRefChunk<A>> + '_ {
        self.ealloc
            .iter_allocated_chunks()
            .map(|range| entity::TempRefChunk::new(range.start, range.end))
    }

    /// Iterates over all entities, yielding the components requested.
    pub fn entities_with<IntoZ: IntoZip<A>>(
        &self,
        zip: IntoZ,
    ) -> impl Iterator<Item = (entity::TempRef<A>, <IntoZ::IntoZip as Zip<A>>::Item)> {
        let mut zip = ZipIter(zip.into_zip(), PhantomData);
        self.ealloc
            .iter_allocated_chunks()
            .flat_map(<A::RawEntity as entity::Raw>::range)
            .map(move |entity| (entity::TempRef::new(entity), zip.take_serial(entity)))
    }

    /// Iterates over all entities, yielding the components requested in contiguous chunks.
    pub fn chunks_with<IntoZ: IntoZip<A>>(
        &self,
        zip: IntoZ,
    ) -> impl Iterator<Item = (entity::TempRefChunk<A>, <IntoZ::IntoZip as ZipChunked<A>>::Chunk)>
    where
        IntoZ::IntoZip: ZipChunked<A>,
    {
        let mut zip = ZipIter(zip.into_zip(), PhantomData);
        self.ealloc.iter_allocated_chunks().map(move |chunk| {
            (
                entity::TempRefChunk::new(chunk.start, chunk.end),
                zip.take_serial_chunk(chunk.start, chunk.end),
            )
        })
    }
}

struct ZipIter<A: Archetype, Z: Zip<A>>(Z, PhantomData<A>);

impl<A: Archetype, Z: Zip<A>> ZipIter<A, Z> {
    fn take_serial(&mut self, entity: A::RawEntity) -> Z::Item {
        let right = self.0.split(entity.add(1)); // add 1 so that `entity` remains on the left chunk
        let left = mem::replace(&mut self.0, right);
        left.get(entity::TempRef::new(entity))
    }
}

impl<A: Archetype, Z: ZipChunked<A>> ZipIter<A, Z> {
    fn take_serial_chunk(&mut self, start: A::RawEntity, end: A::RawEntity) -> Z::Chunk {
        let right = self.0.split(end); // no need to add 1 here since `end` does not belong to the required chunk
        let left = mem::replace(&mut self.0, right);
        left.get_chunk(entity::TempRefChunk::new(start, end))
    }
}

/// Multiple single accessors zipped together,
/// to be used with [`EntityIterator::entities_with`](crate::system::EntityIterator::entities_with).
///
/// All accessors must target the same archetype `A`.
///
/// See [`IntoZip`] for what values can be passed for `Zip`.
pub trait Zip<A: Archetype>: Sized {
    /// Vertically splits each underlying storage vertically (by entities) at `offset`.
    fn split(&mut self, offset: A::RawEntity) -> Self;

    /// The type of values available for a single entity.
    type Item;
    /// Returns the requested components for the specified entity.
    fn get<E: entity::Ref<Archetype = A>>(self, entity: E) -> Self::Item;
}

/// [`Zip`] accessors with the additional condition that
/// all underlying storages support chunked access,
/// to be used with [`EntityIterator::chunks_with`](crate::system::EntityIterator::chunks_with).
pub trait ZipChunked<A: Archetype>: Zip<A> {
    /// The type of values available for a single chunk.
    type Chunk;
    /// Returns the requested components as chunks for the specified entities.
    fn get_chunk(self, chunk: entity::TempRefChunk<A>) -> Self::Chunk;
}

/// Values that can be used as a [`Zip`] in [`EntityIterator`](crate::system::EntityIterator),
/// similar to [`IntoIterator`] for iterators.
///
/// This trait is intended to map storages to components of a single entity,
/// so it is implemented by:
/// - [`&ReadSimple`](crate::system::ReadSimple) and [`&mut WriteSimple`](crate::system::WriteSimple)
/// - Shared/mutable references to [split](access::Isotope::split) isotope accessors
/// - Any of the above wrapped with [`Try`] for [optional](comp::Presence::Optional) components.
/// - Tuples of `Zip` implementors, including other tuples.
/// - Structs of `Zip` fields that use the [`Zip`](crate::zip) derive macro.
///
/// The default configuration only implements for tuples of up to 4 elements.
/// To use larger tuples at the cost of slower compile time,
/// use the feature `"tuple-impl-{n}-zip"`,
/// where `{n}` is `8`, `16`, `24` or `32`.
pub trait IntoZip<A: Archetype> {
    /// The [`Zip`] type that this is converted into.
    type IntoZip: Zip<A>;
    /// Converts into a [`Zip`] object.
    fn into_zip(self) -> Self::IntoZip;
}

/// Determines how to resolve the case of a missing Result.
pub trait MissingResln {
    /// The return type of the resolution.
    type Result<T>;
    /// Resolves an optional value.
    fn must_or_try<T>(option: Option<T>) -> Self::Result<T>;
}

/// Automatically unwraps storage results.
pub struct MustMissingResln<A: Archetype, C: comp::Must<A>>(PhantomData<(A, C)>);
impl<A: Archetype, C: comp::Must<A>> MissingResln for MustMissingResln<A, C> {
    type Result<T> = T;
    fn must_or_try<T>(option: Option<T>) -> T {
        match option {
            Some(value) => value,
            None => panic!(
                "Component {}/{} implements comp::Must but is not present",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        }
    }
}

/// Returns `None` if component is missing.
pub enum TryMissingResln {}
impl MissingResln for TryMissingResln {
    type Result<T> = Option<T>;
    fn must_or_try<T>(option: Option<T>) -> Option<T> { option }
}

/// Wrap accessor references with `Try` to indicate that the result should be an `Option`.
pub struct Try<T>(pub T);

impl<'t, A, C, AccessorT> IntoZip<A> for Try<&'t AccessorT>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A>,
    AccessorT: single::Get<Arch = A, Comp = C>,
{
    type IntoZip = Read<'t, A, C, AccessorT, TryMissingResln>;
    fn into_zip(self) -> Self::IntoZip { Read { accessor: self.0, _ph: PhantomData } }
}

impl<'t, A, C, AccessorT> IntoZip<A> for &'t AccessorT
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A> + comp::Must<A>,
    AccessorT: single::Get<Arch = A, Comp = C>,
{
    type IntoZip = Read<'t, A, C, AccessorT, MustMissingResln<A, C>>;
    fn into_zip(self) -> Self::IntoZip { Read { accessor: self, _ph: PhantomData } }
}

/// [`IntoZip::IntoZip`] for read-only accessors.
pub struct Read<'t, A, C, AccessorT, Resln> {
    accessor: &'t AccessorT,
    _ph:      PhantomData<(A, C, Resln)>,
}

impl<'t, A, C, AccessorT, Resln> Copy for Read<'t, A, C, AccessorT, Resln> {}
impl<'t, A, C, AccessorT, Resln> Clone for Read<'t, A, C, AccessorT, Resln> {
    fn clone(&self) -> Self { *self }
}

impl<'t, A, C, AccessorT, Resln> Zip<A> for Read<'t, A, C, AccessorT, Resln>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A>,
    AccessorT: single::Get<Arch = A, Comp = C>,
    Resln: MissingResln,
{
    fn split(&mut self, _offset: A::RawEntity) -> Self { *self }

    type Item = Resln::Result<&'t C>;
    fn get<E: entity::Ref<Archetype = A>>(self, entity: E) -> Resln::Result<&'t C> {
        Resln::must_or_try(self.accessor.try_get(entity))
    }
}

impl<'t, A, C, AccessorT> ZipChunked<A> for Read<'t, A, C, AccessorT, MustMissingResln<A, C>>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A> + comp::Must<A>,
    AccessorT: single::Get<Arch = A, Comp = C> + single::GetChunked<Arch = A, Comp = C>,
{
    type Chunk = &'t [C];
    fn get_chunk(self, chunk: entity::TempRefChunk<A>) -> Self::Chunk {
        self.accessor.get_chunk(chunk)
    }
}

impl<'t, A, C, StorageRef> IntoZip<A> for Try<&'t mut access::Single<A, C, StorageRef>>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A>,
    StorageRef: ops::DerefMut + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
{
    type IntoZip = Write<
        't,
        A,
        C,
        util::OwnedDeref<<StorageRef::Target as Storage>::Partition<'t>>,
        TryMissingResln,
    >;
    fn into_zip(self) -> Self::IntoZip {
        Write { accessor: self.0.as_partition(), _ph: PhantomData }
    }
}

impl<'t, A, C, StorageRef> IntoZip<A> for &'t mut access::Single<A, C, StorageRef>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A> + comp::Must<A>,
    StorageRef: ops::DerefMut + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
{
    type IntoZip = Write<
        't,
        A,
        C,
        util::OwnedDeref<<StorageRef::Target as Storage>::Partition<'t>>,
        MustMissingResln<A, C>,
    >;
    fn into_zip(self) -> Self::IntoZip { Write { accessor: self.as_partition(), _ph: PhantomData } }
}

/// [`IntoZip::IntoZip`] for mutable accessors.
pub struct Write<'t, A, C, PartitionT, Resln> {
    accessor: access::Single<A, C, PartitionT>,
    _ph:      PhantomData<(&'t mut C, Resln)>,
}

impl<'t, A, C, PartitionT, Resln> Zip<A> for Write<'t, A, C, util::OwnedDeref<PartitionT>, Resln>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A>,
    PartitionT: storage::Partition<'t, RawEntity = A::RawEntity, Comp = C>,
    Resln: MissingResln,
{
    fn split(&mut self, offset: A::RawEntity) -> Self {
        let right = self.accessor.split_out(offset);
        Self { accessor: right, _ph: PhantomData }
    }

    type Item = Resln::Result<&'t mut C>;
    fn get<E: entity::Ref<Archetype = A>>(self, entity: E) -> Resln::Result<&'t mut C> {
        Resln::must_or_try(self.accessor.try_into_mut(entity))
    }
}

impl<'t, A, C, PartitionT> ZipChunked<A>
    for Write<'t, A, C, util::OwnedDeref<PartitionT>, MustMissingResln<A, C>>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A> + comp::Must<A>,
    PartitionT: storage::PartitionChunked<'t, RawEntity = A::RawEntity, Comp = C>,
{
    type Chunk = &'t mut [C];
    fn get_chunk(self, chunk: entity::TempRefChunk<A>) -> Self::Chunk {
        self.accessor.into_chunk_mut(chunk)
    }
}

mod tuple_impls;

#[cfg(test)]
mod tests;