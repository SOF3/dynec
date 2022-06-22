use std::marker::PhantomData;
use std::mem::{self, MaybeUninit};

use bitvec::prelude::BitVec;

use super::Storage;
use crate::entity;

/// The basic storage indexed by entity IDs directly.
pub struct VecStorage<E: entity::Raw, T> {
    bits: BitVec,
    data: Vec<MaybeUninit<T>>,
    _ph:  PhantomData<E>,
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

        self.bits.set(index, bit);
    }
}

impl<E: entity::Raw, T> Default for VecStorage<E, T> {
    fn default() -> Self { Self { bits: BitVec::new(), data: Vec::new(), _ph: PhantomData } }
}

impl<E: entity::Raw, C: Send + Sync + 'static> Storage for VecStorage<E, C> {
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

    fn iter(&self) -> Box<dyn Iterator<Item = (E, &C)> + '_> {
        let indices = self.bits.iter_ones();
        let data = &self.data;

        Box::new(indices.map(move |index| {
            let entity = E::from_primitive(index);
            let value = data.get(index).expect("bits mismatch");
            let value = unsafe { value.assume_init_ref() };
            (entity, value)
        }))
    }

    fn iter_mut(&mut self) -> Box<dyn Iterator<Item = (E, &mut C)> + '_> {
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
}
