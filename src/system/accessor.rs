//! Abstraction of entity storages for iteration.

use std::marker::PhantomData;
use std::ops;

use super::{rw, Read};
use crate::storage::Chunked as _;
use crate::{comp, entity, storage, Archetype};

/// An accessor that can be used in an entity iteration.
///
/// # Safety
/// Implementors must ensure that [`entity`](Self::entity) is deterministic and [one-to-one][injective].
///
/// Multiplexing implementors (such as tuples or composite accessors)
/// preserve this invariant automatically since they are just destructuring to independent storages.
/// Storage delegations preserve this invariant automatically
/// since [`Storage::get_mut`](crate::storage::Storage::get_mut)
/// has the same safety invariants
/// (see [`Storage` &sect; Safety](crate::storage::Storage#safety)).
///
/// [injective]: https://en.wikipedia.org/wiki/Injective_function
pub unsafe trait Accessor<A: Archetype> {
    /// Return value of [`entity`](Self::entity).
    type Entity<'t>: 't
    where
        Self: 't;
    /// Accesses this storage for a specific entity.
    ///
    /// # Safety
    /// The lifetime of the return value is arbitrarily defined by the caller.
    /// This effectively disables the borrow checker for return values.
    /// The caller must ensure that return values do not outlive `self`,
    /// and the function result is dropped before it is called again with the same `id`.
    unsafe fn entity<'this, 'e, 'ret>(
        this: &'this mut Self,
        id: entity::TempRef<'e, A>,
    ) -> Self::Entity<'ret>
    where
        Self: 'ret;
}

/// An accessor that can be used in chunked entity iteration.
///
/// # Safety
/// Implementors must ensure that [`chunk`](Self::chunk) is deterministic,
/// and non-overlapping entity chunks return non-overlapping values.
/// This is equivalent to (and should delegate to)
/// [`crate::storage::Chunked::get_chunk`]/[`crate::storage::Chunked::get_chunk_mut`].
///
/// Multiplexing implementors (such as tuples or composite accessors)
/// preserve this invariant automatically since they are just destructuring to independent storages.
/// Storage delegations preserve this invariant automatically
/// since [`crate::storage::Chunked::get_chunk_mut`] has the same safety invariants
/// (see [`Chunked` &sect; Safety](crate::storage::Chunked#safety)).
///
/// [injective]: https://en.wikipedia.org/wiki/Injective_function
pub unsafe trait Chunked<A: Archetype> {
    /// Return value of [`chunk`](Self::chunk).
    type Chunk<'t>: 't
    where
        Self: 't;
    /// Accesses this storage for a specific chunk of entities.
    ///
    /// # Safety
    /// The lifetime of the return value is arbitrarily defined by the caller.
    /// This effectively disables the borrow checker for return values.
    /// The caller must ensure that return values do not outlive `self`,
    /// and the function result is dropped before it is called again with an overlapping `chunk`.
    unsafe fn chunk<'ret>(this: &mut Self, chunk: entity::TempRefChunk<'_, A>)
        -> Self::Chunk<'ret>;
}

/// Return value of [`Read::try_access`].
pub struct TryRead<A, C, T>(pub(super) T, pub(super) PhantomData<(A, C)>);

unsafe impl<A, C, T> Accessor<A> for TryRead<A, C, T>
where
    A: Archetype,
    C: 'static,
    T: ops::Deref,
    T::Target: rw::Read<A, C>,
{
    type Entity<'ret> = Option<&'ret C> where Self: 'ret;

    unsafe fn entity<'ret>(this: &mut Self, id: entity::TempRef<'_, A>) -> Self::Entity<'ret>
    where
        Self: 'ret,
    {
        Some(&*(this.0.try_get(id)? as *const C))
    }
}

/// Return value of [`Read::access`].
pub struct MustRead<A, C, T>(pub(super) T, pub(super) PhantomData<(A, C)>);

unsafe impl<'t, A: Archetype, C: comp::Must<A> + 'static, T: rw::Read<A, C>> Accessor<A>
    for MustRead<A, C, &'t T>
{
    type Entity<'ret> = &'ret C where Self: 'ret;

    unsafe fn entity<'this, 'e, 'ret>(
        this: &'this mut Self,
        id: entity::TempRef<'e, A>,
    ) -> Self::Entity<'ret>
    where
        Self: 'ret,
    {
        &*(this.0.get(id) as *const C)
    }
}

pub struct MustReadChunkSimple<'t, A: Archetype, C: comp::Simple<A>> {
    pub(crate) storage: &'t C::Storage,
}

unsafe impl<'t, A: Archetype, C: comp::Simple<A> + comp::Must<A> + 'static> Chunked<A>
    for MustReadChunkSimple<'t, A, C>
where
    C::Storage: storage::Chunked,
{
    type Chunk<'ret> = &'ret [C] where Self: 'ret;

    unsafe fn chunk<'this, 'e, 'ret>(
        this: &'this mut Self,
        chunk: entity::TempRefChunk<'e, A>,
    ) -> Self::Chunk<'ret>
    where
        Self: 'ret,
    {
        &*(this
            .storage
            .get_chunk(chunk.start, chunk.end)
            .expect("TempRefChunk points to missing entities") as *const [C])
    }
}

pub struct MustWriteChunkSimple<'t, A: Archetype, C: comp::Simple<A>> {
    pub(crate) storage: &'t mut C::Storage,
}

unsafe impl<'t, A: Archetype, C: comp::Simple<A> + comp::Must<A> + 'static> Chunked<A>
    for MustWriteChunkSimple<'t, A, C>
where
    C::Storage: storage::Chunked,
{
    type Chunk<'ret> = &'ret mut [C] where Self: 'ret;

    unsafe fn chunk<'this, 'e, 'ret>(
        this: &'this mut Self,
        chunk: entity::TempRefChunk<'e, A>,
    ) -> Self::Chunk<'ret>
    where
        Self: 'ret,
    {
        &mut *(this
            .storage
            .get_chunk_mut(chunk.start, chunk.end)
            .expect("TempRefChunk points to missing entities") as *mut [C])
    }
}

/// Return value of [`system::Write::try_access_mut`](crate::system::Write::try_access_mut).
pub struct TryWrite<A, C, T>(pub(super) T, pub(super) PhantomData<(A, C)>);

unsafe impl<'t, A: Archetype, C: 'static, T: rw::Write<A, C>> Accessor<A>
    for TryWrite<A, C, &'t mut T>
{
    type Entity<'ret> = Option<&'ret mut C> where Self: 'ret;

    unsafe fn entity<'this, 'e, 'ret>(
        this: &'this mut Self,
        id: entity::TempRef<'e, A>,
    ) -> Self::Entity<'ret>
    where
        Self: 'ret,
    {
        Some(&mut *(this.0.try_get_mut(id)? as *mut C))
    }
}

/// Return value of [`system::Write::access_mut`](crate::system::Write::access_mut).
pub struct MustWrite<A, C, T>(pub(super) T, pub(super) PhantomData<(A, C)>);

unsafe impl<'t, A: Archetype, C: comp::Must<A> + 'static, T: rw::Write<A, C>> Accessor<A>
    for MustWrite<A, C, &'t mut T>
{
    type Entity<'ret> = &'ret mut C where Self: 'ret;

    unsafe fn entity<'this, 'e, 'ret>(
        this: &'this mut Self,
        id: entity::TempRef<'e, A>,
    ) -> Self::Entity<'ret>
    where
        Self: 'ret,
    {
        &mut *(this.0.get_mut(id) as *mut C)
    }
}

unsafe impl<'t, A: Archetype, C: comp::Must<A> + 'static, T: rw::MutChunk<A, C>> Chunked<A>
    for MustWrite<A, C, &'t mut T>
{
    type Chunk<'ret> = &'ret mut [C] where Self: 'ret;

    unsafe fn chunk<'this, 'e, 'ret>(
        this: &'this mut Self,
        chunk: entity::TempRefChunk<'e, A>,
    ) -> Self::Chunk<'ret>
    where
        Self: 'ret,
    {
        &mut *(this.0.get_chunk_mut(chunk) as *mut [C])
    }
}

mod tuple_impl;
