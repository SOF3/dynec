use std::marker::PhantomData;
use std::{any, ops};

use super::AccessSingle;
use crate::{comp, entity, storage, util, Archetype, Storage};

/// Multiple single accessors zipped together.
pub trait Zip: Sized {
    type Archetype: Archetype;

    fn split(&mut self, offset: <Self::Archetype as Archetype>::RawEntity) -> Self;

    type Item;
    fn get<E: entity::Ref<Archetype = Self::Archetype>>(self, entity: E) -> Self::Item;
}

pub trait ZipChunked: Zip {
    type Chunk;
    fn get_chunk(self, entity: entity::TempRefChunk<Self::Archetype>) -> Self::Chunk;
}

pub trait IntoZip {
    type Archetype: Archetype;

    type IntoZip: Zip<Archetype = Self::Archetype>;
    fn into_zip(self) -> Self::IntoZip;
}

impl<T: Zip> IntoZip for T {
    type Archetype = <Self as Zip>::Archetype;

    type IntoZip = Self;
    fn into_zip(self) -> Self { self }
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

impl<'t, A, C, StorageRef> IntoZip for Try<&'t AccessSingle<A, C, StorageRef>>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A>,
    StorageRef: ops::Deref + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
{
    type Archetype = A;

    type IntoZip = Read<'t, A, C, StorageRef, TryMissingResln>;
    fn into_zip(self) -> Self::IntoZip { Read { accessor: self.0, _ph: PhantomData } }
}

impl<'t, A, C, StorageRef> IntoZip for &'t AccessSingle<A, C, StorageRef>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A> + comp::Must<A>,
    StorageRef: ops::Deref + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
{
    type Archetype = A;

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

impl<'t, A, C, StorageRef, Resln> Zip for Read<'t, A, C, StorageRef, Resln>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A>,
    StorageRef: ops::Deref + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
    Resln: MissingResln,
{
    type Archetype = A;

    fn split(&mut self, _offset: A::RawEntity) -> Self { *self }

    type Item = Resln::Result<&'t C>;
    fn get<E: entity::Ref<Archetype = Self::Archetype>>(self, entity: E) -> Resln::Result<&'t C> {
        Resln::must_or_try(self.accessor.try_get(entity))
    }
}

impl<'t, A, C, StorageRef> IntoZip for Try<&'t mut AccessSingle<A, C, StorageRef>>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A>,
    StorageRef: ops::Deref + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
{
    type Archetype = A;

    type IntoZip = Read<'t, A, C, StorageRef, TryMissingResln>;
    fn into_zip(self) -> Self::IntoZip { Read { accessor: self.0, _ph: PhantomData } }
}

impl<'t, A, C, StorageRef> IntoZip for &'t mut AccessSingle<A, C, StorageRef>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A> + comp::Must<A>,
    StorageRef: ops::DerefMut + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
{
    type Archetype = A;

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

impl<'t, A, C, PartitionT, Resln> Zip for Write<'t, A, C, util::OwnedDeref<PartitionT>, Resln>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A>,
    PartitionT: storage::Partition<'t, RawEntity = A::RawEntity, Comp = C>,
    Resln: MissingResln,
{
    type Archetype = A;

    fn split(&mut self, offset: A::RawEntity) -> Self {
        let right = self.accessor.split_out(offset);
        Self { accessor: right, _ph: PhantomData }
    }

    type Item = Resln::Result<&'t mut C>;
    fn get<E: entity::Ref<Archetype = Self::Archetype>>(
        self,
        entity: E,
    ) -> Resln::Result<&'t mut C> {
        Resln::must_or_try(self.accessor.try_into_mut(entity))
    }
}

impl<'t, A, C, PartitionT> ZipChunked
    for Write<'t, A, C, util::OwnedDeref<PartitionT>, MustMissingResln<A, C>>
where
    A: Archetype,
    C: comp::SimpleOrIsotope<A> + comp::Must<A>,
    PartitionT: storage::PartitionChunked<'t, RawEntity = A::RawEntity, Comp = C>,
{
    type Chunk = &'t mut [C];
    fn get_chunk(self, chunk: entity::TempRefChunk<Self::Archetype>) -> Self::Chunk {
        self.accessor.into_chunk_mut(chunk)
    }
}
