use std::marker::PhantomData;
use std::{any, ops};

use super::AccessSingle;
use crate::{comp, entity, storage, util, Archetype, Storage};

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

/// Values that can be used as a [`Zip`] in [`EntityIterator`],
/// similar to [`IntoIterator`] for iterators.
///
/// This trait is intended to map storages to components of a single entity,
/// so it is implemented by:
/// - [`&ReadSimple`](crate::system::ReadSimple) and [`&mut WriteSimple`](crate::system::WriteSimple)
/// - Shared/mutable references to [split](super::AccessIsotope::split) isotope accessors
/// - Any of the above wrapped with [`Try`] for [optional](comp::Presence::Optional) components.
/// - Tuples of `Zip` implementors, including other tuples.
/// - Structs of `Zip` fields that use the [`Zip`](crate::Zip) derive macro.
pub trait IntoZip<A: Archetype> {
    /// The [`Zip`] type that this is converted into.
    type IntoZip: Zip<A>;
    /// Converts into a [`Zip`] object.
    fn into_zip(self) -> Self::IntoZip;
}

/// Either returns Option or unwraps it.
pub trait MissingResln {
    type Result<T>;
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

impl<'t, A, C, StorageRef> IntoZip<A> for Try<&'t AccessSingle<A, C, StorageRef>>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A>,
    StorageRef: ops::Deref + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
{
    type IntoZip = Read<'t, A, C, StorageRef, TryMissingResln>;
    fn into_zip(self) -> Self::IntoZip { Read { accessor: self.0, _ph: PhantomData } }
}

impl<'t, A, C, StorageRef> IntoZip<A> for &'t AccessSingle<A, C, StorageRef>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A> + comp::Must<A>,
    StorageRef: ops::Deref + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
{
    type IntoZip = Read<'t, A, C, StorageRef, MustMissingResln<A, C>>;
    fn into_zip(self) -> Self::IntoZip { Read { accessor: self, _ph: PhantomData } }
}

pub struct Read<'t, A, C, StorageRef, Resln> {
    accessor: &'t AccessSingle<A, C, StorageRef>,
    _ph:      PhantomData<Resln>,
}

impl<'t, A, C, StorageRef, Resln> Copy for Read<'t, A, C, StorageRef, Resln> {}
impl<'t, A, C, StorageRef, Resln> Clone for Read<'t, A, C, StorageRef, Resln> {
    fn clone(&self) -> Self { *self }
}

impl<'t, A, C, StorageRef, Resln> Zip<A> for Read<'t, A, C, StorageRef, Resln>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A>,
    StorageRef: ops::Deref + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
    Resln: MissingResln,
{
    fn split(&mut self, _offset: A::RawEntity) -> Self { *self }

    type Item = Resln::Result<&'t C>;
    fn get<E: entity::Ref<Archetype = A>>(self, entity: E) -> Resln::Result<&'t C> {
        Resln::must_or_try(self.accessor.try_get(entity))
    }
}

impl<'t, A, C, StorageRef> ZipChunked<A> for Read<'t, A, C, StorageRef, MustMissingResln<A, C>>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A> + comp::Must<A>,
    StorageRef: ops::Deref + Sync,
    StorageRef::Target: storage::Chunked<RawEntity = A::RawEntity, Comp = C>,
{
    type Chunk = &'t [C];
    fn get_chunk(self, chunk: entity::TempRefChunk<A>) -> Self::Chunk {
        self.accessor.get_chunk(chunk)
    }
}

impl<'t, A, C, StorageRef> IntoZip<A> for Try<&'t mut AccessSingle<A, C, StorageRef>>
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

impl<'t, A, C, StorageRef> IntoZip<A> for &'t mut AccessSingle<A, C, StorageRef>
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

pub struct Write<'t, A, C, PartitionT, Resln> {
    accessor: AccessSingle<A, C, PartitionT>,
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
