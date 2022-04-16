use std::any::TypeId;
use std::cmp;
use std::collections::{hash_map, HashMap};

use xias::Xias;

use crate::entity;

/// A permutation of entities.
pub struct Permutation {
    pub index: Vec<entity::Raw>,
}

impl Permutation {
    pub fn from_comparator(
        positions: impl IntoIterator<Item = entity::Raw>,
        mut comparator: impl FnMut(entity::Raw, entity::Raw) -> cmp::Ordering,
    ) -> Self {
        let mut inverse: Vec<_> = positions.into_iter().collect();
        if inverse.is_empty() {
            return Self { index: vec![] };
        }

        inverse.sort_unstable_by(|&a, &b| comparator(a, b));

        let index_len =
            inverse.iter().copied().max().expect("inverse is nonempty").0.small_int::<usize>() + 1;

        let mut index = vec![entity::Raw(0); index_len];
        for (target, original) in inverse.iter().enumerate() {
            let entry = index
                .get_mut(original.0.small_int::<usize>())
                .expect("index_len > all inverse values");
            *entry = entity::Raw(target.small_int());
        }

        Self { index }
    }

    /// Validates whether the permutation is a bijection.
    pub fn validate(
        &self,
        had_original_index: impl Fn(entity::Raw) -> bool,
    ) -> Result<(), ValidationError> {
        #[cfg(debug_assertions)]
        {
            let mut inverse = HashMap::with_capacity(self.index.len());
            for (original, &target) in self.index.iter().enumerate() {
                let original = entity::Raw(original.small_int());

                if target.0.small_int::<usize>() >= self.index.len() {
                    return Err(ValidationError::OutOfRange { position: original, target });
                }

                if had_original_index(original) {
                    match inverse.entry(original) {
                        hash_map::Entry::Occupied(entry) => {
                            return Err(ValidationError::Duplicate {
                                positions: [original, *entry.get()],
                                target,
                            });
                        }
                        hash_map::Entry::Vacant(entry) => {
                            entry.insert(original);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Updates an entity referrer with the permutation.
    pub fn update_referrer(&self, archetype: TypeId, referrer: &mut impl entity::Referrer) {
        referrer.visit(archetype, &mut |r| self.update(r))
    }

    /// Updates an entity reference with the permutation.
    pub fn update(&self, entity: &mut entity::Raw) {
        let original = *entity;
        let new =
            self.index.get(original.0.small_int::<usize>()).expect("Permutation out of bounds");
        *entity = *new;
    }
}

/// An error returned when the permutation is invalid.
pub enum ValidationError {
    OutOfRange {
        /// The original index.
        position: entity::Raw,
        /// The out-of-range index.
        target:   entity::Raw,
    },
    /// Multiple original indices point to the same target.
    Duplicate {
        /// The two original indices that alias the same target.
        positions: [entity::Raw; 2],
        /// The target index.
        target:    entity::Raw,
    },
}
