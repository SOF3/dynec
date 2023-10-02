use std::iter;
use std::marker::PhantomData;
use std::mem::{self, MaybeUninit};

use bitvec::prelude::BitVec;
use bitvec::slice::BitSlice;

use super::{
    Access, AccessChunked, ChunkMut, ChunkRef, Chunked, Partition, PartitionChunked, Storage,
};
use crate::{entity, util};

/// The basic storage indexed by entity IDs directly.
pub struct VecStorage<E: entity::Raw, T> {
    cardinality: usize,
    bits:        BitVec,
    data:        Vec<MaybeUninit<T>>,
    _ph:         PhantomData<E>,
}

impl<E: entity::Raw, T> VecStorage<E, T> {
    fn bit(&self, index: usize) -> bool {
        match self.bits.get(index) {
            Some(bit) => *bit,
            None => false,
        }
    }

    fn set_bit(&mut self, index: usize, bit: bool) {
        if self.bits.len() <= index {
            self.bits.resize(index + 1, false);
        }

        let delta_old = match *self.bits.get(index).expect("resized len >= index+1") {
            false => 0,
            true => 1,
        };
        let delta_new = match bit {
            false => 0,
            true => 1,
        };
        self.bits.set(index, bit);

        // split into two separate statements to avoid integer underflow
        self.cardinality -= delta_old;
        self.cardinality += delta_new;
    }
}

impl<E: entity::Raw, T> Default for VecStorage<E, T> {
    fn default() -> Self {
        Self {
            cardinality: 0,
            bits:        BitVec::new(),
            data:        Vec::new(),
            _ph:         PhantomData,
        }
    }
}

impl<E: entity::Raw, C: Send + Sync + 'static> Access for VecStorage<E, C> {
    type RawEntity = E;
    type Comp = C;

    fn get_mut(&mut self, id: E) -> Option<&mut C> {
        let index = id.to_primitive();

        if self.bit(index) {
            let value = self.data.get_mut(index).expect("bits mismatch");
            let value = unsafe { value.assume_init_mut() };
            Some(value)
        } else {
            None
        }
    }

    type IterMut<'t> = impl Iterator<Item = (E, &'t mut C)> + 't;
    fn iter_mut(&mut self) -> Self::IterMut<'_> { iter_mut(0, &self.bits, &mut self.data) }
}

impl<E: entity::Raw, C: Send + Sync + 'static> Storage for VecStorage<E, C> {
    fn get(&self, id: E) -> Option<&C> {
        let index = id.to_primitive();

        if self.bit(index) {
            let value = self.data.get(index).expect("bits mismatch");
            let value = unsafe { value.assume_init_ref() };
            Some(value)
        } else {
            None
        }
    }

    fn set(&mut self, id: E, new: Option<C>) -> Option<C> {
        let index = id.to_primitive();

        let old = if self.bit(index) {
            let value = self.data.get(index).expect("bits mismatch");
            let value = unsafe { value.assume_init_read() };
            Some(value)
        } else {
            None
        };

        // the original value was already moved out, now we can overwrite the data or unmark it

        match new {
            Some(new) => {
                self.set_bit(index, true);
                if self.data.len() <= index {
                    self.data.resize_with(index + 1, MaybeUninit::uninit);
                }
                let bytes = self.data.get_mut(index).expect("just resized");
                *bytes = MaybeUninit::new(new);
            }
            None => {
                self.set_bit(index, false);
            }
        }

        old
    }

    fn cardinality(&self) -> usize { self.cardinality }

    type Iter<'t> = impl Iterator<Item = (E, &'t C)> + 't;
    fn iter(&self) -> Self::Iter<'_> {
        let indices = self.bits.iter_ones();
        let data = &self.data;

        indices.map(move |index| {
            let entity = E::from_primitive(index);
            let value = data.get(index).expect("bits mismatch");
            let value = unsafe { value.assume_init_ref() };
            (entity, value)
        })
    }

    type IterChunks<'t> = impl Iterator<Item = ChunkRef<'t, Self>> + 't;
    fn iter_chunks(&self) -> Self::IterChunks<'_> {
        new_iter_chunks_ref(&self.bits, &self.data[..]).map(|(start_index, chunk)| ChunkRef {
            slice: unsafe { slice_assume_init_ref(chunk) },
            start: E::from_primitive(start_index),
        })
    }

    type IterChunksMut<'t> = impl Iterator<Item = ChunkMut<'t, Self>> + 't;
    fn iter_chunks_mut(&mut self) -> Self::IterChunksMut<'_> {
        new_iter_chunks_mut(&self.bits, &mut self.data[..]).map(|(start_index, chunk)| ChunkMut {
            slice: unsafe { slice_assume_init_mut(chunk) },
            start: E::from_primitive(start_index),
        })
    }

    type Partition<'t> = StoragePartition<'t, E, C>;
    fn as_partition(&mut self) -> Self::Partition<'_> { self.as_partition_chunk() }
}

fn iter_mut<'storage, E: entity::Raw, C: 'static>(
    start_offset: usize,
    bits: &'storage bitvec::slice::BitSlice,
    data: &'storage mut [MaybeUninit<C>],
) -> impl Iterator<Item = (E, &'storage mut C)> + 'storage {
    let indices = bits.iter_ones();

    indices.map(move |index| {
        let entity = E::from_primitive(start_offset + index);
        let value = data.get_mut(index).expect("bits mismatch");
        let value = unsafe { value.assume_init_mut() };
        let value = unsafe { mem::transmute::<&mut C, &mut C>(value) };
        (entity, value)
    })
}

/// Return value of [`VecStorage::split_at`].
pub struct StoragePartition<'t, E: entity::Raw, C> {
    bits:   &'t BitSlice,
    data:   &'t mut [MaybeUninit<C>],
    offset: usize,
    _ph:    PhantomData<E>,
}

impl<'t, E: entity::Raw, C: Send + Sync + 'static> Access for StoragePartition<'t, E, C> {
    type RawEntity = E;
    type Comp = C;

    fn get_mut(&mut self, entity: E) -> Option<&mut C> { self.by_ref().into_mut(entity) }

    type IterMut<'u> = impl Iterator<Item = (E, &'u mut C)> + 'u where Self: 'u;
    fn iter_mut(&mut self) -> Self::IterMut<'_> { self.by_ref().into_iter_mut() }
}

impl<'t, E: entity::Raw, C: Send + Sync + 'static> Partition<'t> for StoragePartition<'t, E, C> {
    type ByRef<'u> = StoragePartition<'u, E, C> where Self: 'u;
    fn by_ref(&mut self) -> Self::ByRef<'_> {
        StoragePartition {
            bits:   self.bits,
            data:   &mut *self.data,
            offset: self.offset,
            _ph:    PhantomData,
        }
    }

    type IntoIterMut = impl Iterator<Item = (E, &'t mut C)>;
    fn into_iter_mut(self) -> Self::IntoIterMut { iter_mut(self.offset, self.bits, self.data) }

    fn into_mut(self, entity: E) -> Option<&'t mut C> {
        let index = match entity.to_primitive().checked_sub(self.offset) {
            Some(index) => index,
            None => panic!("Entity {entity:?} is not in the partition {:?}..", self.offset),
        };
        match self.bits.get(index) {
            Some(bit) if *bit => {
                let value = self.data.get_mut(index).expect("bits mismatch");
                Some(unsafe { value.assume_init_mut() })
            }
            Some(_) => None,
            None => panic!(
                "Entity {entity:?} is not in the partition ..{:?}",
                self.offset + self.bits.len()
            ),
        }
    }

    fn split_out(&mut self, entity: E) -> Self {
        let index =
            entity.to_primitive().checked_sub(self.offset).expect("parameter out of bounds");
        assert!(
            index <= self.bits.len(),
            "split at {index} for partition {}..{}",
            self.offset,
            self.offset + self.bits.len()
        );

        let (bits_left, bits_right) = self.bits.split_at(index);
        self.bits = bits_left;

        let data_right = self.data.take_mut(index..).expect("index < self.data.len()");

        Self {
            bits:   bits_right,
            data:   data_right,
            offset: self.offset + index,
            _ph:    PhantomData,
        }
    }
}

impl<E: entity::Raw, C: Send + Sync + 'static> AccessChunked for VecStorage<E, C> {
    fn get_chunk_mut(&mut self, start: E, end: E) -> Option<&mut [C]> {
        let range = start.to_primitive()..end.to_primitive();
        let bits = match self.bits.get(range.clone()) {
            Some(bits) => bits,
            None => return None,
        };
        if !bits.all() {
            return None;
        }

        let data = self
            .data
            .get_mut(range)
            .expect("range exists in self.bits implies existence in self.data");
        Some(unsafe { slice_assume_init_mut(data) })
    }
}

impl<E: entity::Raw, C: Send + Sync + 'static> Chunked for VecStorage<E, C> {
    fn get_chunk(&self, start: E, end: E) -> Option<&[C]> {
        let range = start.to_primitive()..end.to_primitive();
        let bits = match self.bits.get(range.clone()) {
            Some(bits) => bits,
            None => return None,
        };
        if !bits.all() {
            return None;
        }

        let data =
            self.data.get(range).expect("range exists in self.bits implies existence in self.data");
        Some(unsafe { slice_assume_init_ref(data) })
    }

    type PartitionChunked<'u> = Self::Partition<'u>;
    fn as_partition_chunk(&mut self) -> Self::PartitionChunked<'_> {
        StoragePartition {
            bits:   &self.bits,
            data:   &mut self.data,
            offset: 0,
            _ph:    PhantomData,
        }
    }
}

impl<'t, E: entity::Raw, C: Send + Sync + 'static> AccessChunked for StoragePartition<'t, E, C> {
    fn get_chunk_mut(&mut self, start: E, end: E) -> Option<&mut [C]> {
        self.by_ref().into_chunk_mut(start, end)
    }
}

impl<'t, E: entity::Raw, C: Send + Sync + 'static> PartitionChunked<'t>
    for StoragePartition<'t, E, C>
{
    fn into_chunk_mut(self, start: E, end: E) -> Option<&'t mut [C]> {
        let (start, end) = (start.to_primitive() - self.offset, end.to_primitive() - self.offset);
        let range = start..end;

        let bits = match self.bits.get(range.clone()) {
            Some(bits) => bits,
            None => return None,
        };
        if !bits.all() {
            return None;
        }

        let data = self
            .data
            .get_mut(range)
            .expect("range exists in self.bits implies existence in self.data");
        Some(unsafe { slice_assume_init_mut(data) })
    }

    type IntoIterChunksMut = impl Iterator<Item = (E, &'t mut [C])>;
    fn into_iter_chunks_mut(self) -> Self::IntoIterChunksMut {
        // check correctness:
        // `bits[i]` corresponds to `self.data[i]`, of which the index `i` matches `(last_zero ?? -1) + 1 + i`
        let iter = new_iter_chunks_mut(self.bits, self.data);
        let offset = self.offset;
        iter.map(move |(start_index, chunk)| {
            (E::from_primitive(start_index + offset), unsafe { slice_assume_init_mut(chunk) })
        })
    }
}

struct IterChunks<IterZerosT, DataT, TrisplitFn> {
    /// The position of the last value yielded by `iter_zeros`.
    /// Initially always None, which is semantically the same as `-1`.
    last_zero:  Option<usize>,
    /// Result of `bitslice.iter_zeros()`
    iter_zeros: IterZerosT,
    /// A mutable or shared slice containing data.
    ///
    /// `data[last_zero + 1 + i]` must be uninitialized if and only if `iter_zeros` yields `i`.
    data:       DataT,
    /// A function that splits a `data` slice into three parts at a given `index`,
    /// with lengths `index`, `1`, `data.len() - 1 - index`.
    trisplit:   TrisplitFn,
}

fn new_iter_chunks<'t, DataT, TrisplitFn>(
    bits: &'t BitSlice,
    data: DataT,
    trisplit: TrisplitFn,
) -> impl Iterator<Item = (usize, DataT)> + 't
where
    DataT: Default + 't,
    TrisplitFn: Fn(DataT, usize) -> (DataT, DataT, DataT) + 'static,
{
    IterChunks {
        last_zero: None,
        iter_zeros: bits.iter_zeros().chain(iter::once(bits.len())),
        data,
        trisplit,
    }
}
fn new_iter_chunks_ref<'iter, 'data: 'iter, C: 'static>(
    bits: &'iter BitSlice,
    data: &'data [C],
) -> impl Iterator<Item = (usize, &'data [C])> + 'iter {
    new_iter_chunks(bits, data, trisplit_fn_ref)
}
fn new_iter_chunks_mut<'iter, 'data: 'iter, C: 'static>(
    bits: &'iter BitSlice,
    data: &'data mut [C],
) -> impl Iterator<Item = (usize, &'data mut [C])> + 'iter {
    new_iter_chunks(bits, data, trisplit_fn_mut)
}

impl<IterZerosT, DataT, TrisplitFn> Iterator for IterChunks<IterZerosT, DataT, TrisplitFn>
where
    IterZerosT: Iterator<Item = usize>,
    DataT: Default,
    TrisplitFn: Fn(DataT, usize) -> (DataT, DataT, DataT),
{
    type Item = (usize, DataT);

    fn next(&mut self) -> Option<Self::Item> {
        // when next() is not executing, data[0] must correspond to (last_zero ?? -1) + 1, which
        // must also align with `iter_zeros` indices.

        loop {
            let first_one = match self.last_zero {
                Some(index) => index + 1,
                None => 0,
            };

            let next_zero = self.iter_zeros.next()?;
            self.last_zero = Some(next_zero);

            if first_one == next_zero {
                util::transform_mut(&mut self.data, DataT::default(), |data| {
                    let (_empty, _current, rest) = (self.trisplit)(data, 0);
                    (rest, ())
                });
                continue; // empty chunk, skip one item
            }

            let chunk = util::transform_mut(&mut self.data, DataT::default(), |data| {
                let (chunk, _zero, rest) = (self.trisplit)(data, next_zero - first_one);
                (rest, chunk)
            });

            break Some((first_one, chunk));
        }
    }
}

fn trisplit_fn_ref<T>(data: &[T], index: usize) -> (&[T], &[T], &[T]) {
    let (left, rest) = data.split_at(index);
    let (mid, right) = if rest.is_empty() { (&[][..], &[][..]) } else { rest.split_at(1) };
    (left, mid, right)
}
fn trisplit_fn_mut<T>(data: &mut [T], index: usize) -> (&mut [T], &mut [T], &mut [T]) {
    let (left, rest) = data.split_at_mut(index);
    let (mid, right) =
        if rest.is_empty() { (&mut [][..], &mut [][..]) } else { rest.split_at_mut(1) };
    (left, mid, right)
}

unsafe fn slice_assume_init_ref<T>(slice: &[MaybeUninit<T>]) -> &[T] {
    &*(slice as *const [MaybeUninit<T>] as *const [T])
}
unsafe fn slice_assume_init_mut<T>(slice: &mut [MaybeUninit<T>]) -> &mut [T] {
    &mut *(slice as *mut [MaybeUninit<T>] as *mut [T])
}

#[cfg(test)]
super::tests::test_storage!(CHUNKED VecStorage<std::num::NonZeroU32, i64>);
