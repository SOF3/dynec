use std::mem::{self, ManuallyDrop, MaybeUninit};

use bitvec::prelude::BitVec;

/// A `Vec<Option<T>>`-like data structure with optimized discriminant storage using [`BitVec`].
pub struct OptVec<T> {
    is_set: BitVec,
    data: Vec<MaybeUninit<T>>,
}

impl<T> Default for OptVec<T> {
    fn default() -> Self {
        Self {
            is_set: BitVec::new(),
            data: Vec::new(),
        }
    }
}

impl<T> OptVec<T> {
    /// Creates a new `OptVec` with the given capacity.
    pub fn repeat_none(capacity: usize) -> Self {
        Self {
            is_set: BitVec::repeat(false, capacity),
            data: {
                let mut vec = Vec::<MaybeUninit<T>>::with_capacity(capacity);

                // SAFETY: MaybeUninit<T> is uninitialized, so excess capacity can be used directly
                unsafe { vec.set_len(capacity) }

                vec
            },
        }
    }

    /// Returns the number of elements in the `OptVec`.
    pub fn len(&self) -> usize {
        debug_assert!(self.is_set.len() == self.data.len());
        self.is_set.len()
    }

    /// Returns `true` if the `OptVec` contains no elements.
    pub fn is_empty(&self) -> bool {
        self.is_set.is_empty()
    }

    pub fn resize_at_least(&mut self, new_len: usize) {
        if self.len() >= new_len {
            return;
        }

        self.is_set.resize(new_len, false);

        if new_len > self.data.len() {
            self.data.reserve(new_len - self.data.len());

            // SAFETY: size is reserved, and MaybeUninit does not require initialization
            unsafe { self.data.set_len(new_len) };
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if *self.is_set.get(index)? {
            let data = self.data.get(index).expect("is_set is longer than data");
            // SAFETY: The `is_set` bit is set, so the `data` slot is initialized.
            unsafe { Some(data.assume_init_ref()) }
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if *self.is_set.get(index)? {
            let data = self
                .data
                .get_mut(index)
                .expect("is_set is longer than data");
            // SAFETY: The `is_set` bit is set, so the `data` slot is initialized.
            unsafe { Some(data.assume_init_mut()) }
        } else {
            None
        }
    }

    pub fn replace(&mut self, index: usize, value: Option<T>) -> Option<T> {
        let was_set = *self.is_set.get(index)?;

        {
            let mut proxy = self.is_set.get_mut(index)?;
            *proxy = value.is_some();
        }

        if let Some(value) = value {
            let ptr = self
                .data
                .get_mut(index)
                .expect("is_set is longer than data");

            let old = mem::replace(ptr, MaybeUninit::new(value));

            if was_set {
                // SAFETY: The `is_set` bit was set, so the `data` slot is initialized.
                Some(unsafe { old.assume_init() })
            } else {
                None
            }
        } else {
            // no need to update the current value, but we may want to read it

            if was_set {
                // We are copying the bits out of the vec.
                // This should be fine because the `is_set` bit is already set to false,
                // which means we can consume the value.
                //
                // Note that this value must be copied out even if the caller does not use it,
                // because the replaced value needs to be dropped.
                let value = self
                    .data
                    .get_mut(index)
                    .expect("is_set is longer than data");

                // SAFETY: The `is_set` bit was set, so the `data` slot is initialized.
                let value = mem::replace(value, MaybeUninit::uninit());
                Some(unsafe { value.assume_init() })

                // When maybe_uninit_extra is stabilized:
                // Some(unsafe { value.assume_init_read() })
            } else {
                None
            }
        }
    }
}
