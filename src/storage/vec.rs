use std::iter;
use std::marker::PhantomData;
use std::mem::{self, MaybeUninit};

use bitvec::prelude::BitVec;
use bitvec::slice::BitSlice;
use itertools::Itertools;

use super::{ChunkMut, ChunkRef, Chunked, Storage};
use crate::entity;

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

// Safety: the backend of `get`/`get_mut` is a slice.
// Assuming `E` implements Eq + Ord correctly,
// slices are injective because they are simply memory mapping.
unsafe impl<E: entity::Raw, C: Send + Sync + 'static> Storage for VecStorage<E, C> {
    type RawEntity = E;
    type Comp = C;

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
                self.data.resize_with(index + 1, MaybeUninit::uninit);
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

    type Iter<'t> = impl Iterator<Item = (Self::RawEntity, &'t Self::Comp)> + 't;
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
        let tail = self.bits.len(); // add tail to ensure trailing ones get included
        let indices = self.bits.iter_zeros().chain(iter::once(tail));
        let data = &self.data;

        // the first bit is always zero, so no need to worry about the initila `0..from`

        // TODO this unsafe function requires unit tests
        indices
            .tuple_windows()
            .map(|(from, to)| (from + 1)..to)
            .map(move |range| super::ChunkRef {
                slice: unsafe {
                    let slice = data.get(range.clone()).expect("bits mismatch");
                    slice_assume_init_ref(slice)
                },
                start: E::from_primitive(range.start),
            })
            .filter(|chunk| !chunk.slice.is_empty())
    }

    type IterMut<'t> = impl Iterator<Item = (Self::RawEntity, &'t mut Self::Comp)> + 't;
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        let indices = self.bits.iter_ones();
        let data = &mut self.data;

        Box::new(indices.map(move |index| {
            let entity = E::from_primitive(index);
            let value = data.get_mut(index).expect("bits mismatch");
            let value = unsafe { value.assume_init_mut() };
            let value = unsafe { mem::transmute::<&mut C, &mut C>(value) };
            (entity, value)
        }))
    }

    type IterChunksMut<'t> = impl Iterator<Item = ChunkMut<'t, Self>> + 't;
    fn iter_chunks_mut(&mut self) -> Self::IterChunksMut<'_> {
        let tail = self.bits.len(); // add tail to ensure trailing ones get included
        let indices = self.bits.iter_zeros().chain(iter::once(tail));
        let data = &mut self.data;

        // the first bit is always zero, so no need to worry about the initial `0..from`

        // TODO this unsafe function requires unit tests
        indices
            .tuple_windows()
            .map(|(from, to)| (from + 1)..to)
            .map(move |range| ChunkMut {
                slice: unsafe {
                    let uninit = data.get_mut(range.clone()).expect("bits mismatch");
                    let slice = slice_assume_init_mut(uninit);
                    mem::transmute::<&mut [C], &mut [C]>(slice)
                },
                start: E::from_primitive(range.start),
            })
            .filter(|chunk| !chunk.slice.is_empty())
    }

    type StoragePartition<'t> = StoragePartition<'t, E, C>;
    fn partition_at(
        &mut self,
        offset: Self::RawEntity,
    ) -> (StoragePartition<'_, E, C>, StoragePartition<'_, E, C>) {
        let offset = offset.to_primitive();
        let bits = self.bits.split_at(offset);
        let data = self.data.split_at_mut(offset);
        (
            StoragePartition { bits: bits.0, data: data.0, offset: 0, _ph: PhantomData },
            StoragePartition { bits: bits.1, data: data.1, offset, _ph: PhantomData },
        )
    }
}

/// Return value of [`VecStorage::partition_at`].
pub struct StoragePartition<'t, E: entity::Raw, C> {
    bits:   &'t BitSlice,
    data:   &'t mut [MaybeUninit<C>],
    offset: usize,
    _ph:    PhantomData<E>,
}

impl<'t, E: entity::Raw, C: 'static> super::StoragePartition<E, C> for StoragePartition<'t, E, C> {
    fn get_mut(&mut self, entity: E) -> Option<&mut C> {
        let index =
            entity.to_primitive().checked_sub(self.offset).expect("parameter out of bounds");
        match self.bits.get(index) {
            Some(bit) if *bit => {
                let value = self.data.get_mut(index).expect("bits mismatch");
                Some(unsafe { value.assume_init_mut() })
            }
            _ => None,
        }
    }

    type PartitionAt<'u> = StoragePartition<'u, E, C> where Self: 'u;
    fn partition_at(&mut self, entity: E) -> (Self::PartitionAt<'_>, Self::PartitionAt<'_>) {
        let index =
            entity.to_primitive().checked_sub(self.offset).expect("parameter out of bounds");
        assert!(index < self.bits.len());
        let bits = self.bits.split_at(index);
        let data = self.data.split_at_mut(index);
        (
            StoragePartition {
                bits:   bits.0,
                data:   data.0,
                offset: self.offset,
                _ph:    PhantomData,
            },
            StoragePartition {
                bits:   bits.1,
                data:   data.1,
                offset: self.offset + index,
                _ph:    PhantomData,
            },
        )
    }
}

unsafe impl<E: entity::Raw, C: Send + Sync + 'static> Chunked for VecStorage<E, C> {
    fn get_chunk(&self, start: Self::RawEntity, end: Self::RawEntity) -> Option<&[Self::Comp]> {
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

    fn get_chunk_mut(
        &mut self,
        start: Self::RawEntity,
        end: Self::RawEntity,
    ) -> Option<&mut [Self::Comp]> {
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

unsafe fn slice_assume_init_ref<T>(slice: &[MaybeUninit<T>]) -> &[T] {
    &*(slice as *const [MaybeUninit<T>] as *const [T])
}
unsafe fn slice_assume_init_mut<T>(slice: &mut [MaybeUninit<T>]) -> &mut [T] {
    &mut *(slice as *mut [MaybeUninit<T>] as *mut [T])
}
