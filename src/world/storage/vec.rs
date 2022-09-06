use std::marker::PhantomData;
use std::mem::{self, MaybeUninit};

use bitvec::prelude::BitVec;
use itertools::Itertools;

use super::{ChunkMut, ChunkRef, Storage};
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

impl<E: entity::Raw, C: Send + Sync + 'static> Storage for VecStorage<E, C> {
    type RawEntity = E;
    type Comp = C;

    type Iter<'t> = impl Iterator<Item = (Self::RawEntity, &'t Self::Comp)> + 't;
    type IterChunk<'t> = impl Iterator<Item = ChunkRef<'t, Self>> + 't;
    type IterMut<'t> = impl Iterator<Item = (Self::RawEntity, &'t mut Self::Comp)> + 't;
    type IterChunkMut<'t> = impl Iterator<Item = ChunkMut<'t, Self>> + 't;

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

    fn iter_chunks(&self) -> Self::IterChunk<'_> {
        let indices = self.bits.iter_zeros();
        let data = &self.data;

        // the first bit is always zero, so no need to worry about the initila `0..from`

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

    fn iter_chunks_mut(&mut self) -> Self::IterChunkMut<'_> {
        let indices = self.bits.iter_zeros().peekable();
        let data = &mut self.data;

        // the first bit is always zero, so no need to worry about the initila `0..from`

        Box::new(
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
                .filter(|chunk| !chunk.slice.is_empty()),
        )
    }
}

unsafe fn slice_assume_init_ref<T>(slice: &[MaybeUninit<T>]) -> &[T] {
    &*(slice as *const [MaybeUninit<T>] as *const [T])
}
unsafe fn slice_assume_init_mut<T>(slice: &mut [MaybeUninit<T>]) -> &mut [T] {
    &mut *(slice as *mut [MaybeUninit<T>] as *mut [T])
}
