use std::any::TypeId;
use std::cmp;
use std::collections::{hash_map, HashMap};

use xias::Xias;

use crate::entity;

/// A permutation of entities.
pub struct Permutation {
    index: Vec<entity::Raw>,
}

impl Permutation {
    /// Creates a new permutation with a comparator on raw entity positions.
    pub(crate) fn from_comparator(
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

    /// Creates a new permutation by mapping raw entity positions to a comparable type.
    pub(crate) fn from_mapper<T: Ord>(
        positions: impl IntoIterator<Item = entity::Raw>,
        mut mapper: impl FnMut(entity::Raw) -> T,
    ) -> Self {
        Self::from_comparator(positions, |a, b| mapper(a).cmp(&mapper(b)))
    }

    /// A slice that maps entity IDs.
    ///
    /// For each entity at original position `original`,
    /// its new position is `index[original]`.
    pub(crate) fn index(&self) -> &[entity::Raw] { &self.index }

    /// Validates whether the permutation is a bijection.
    pub(crate) fn validate(
        &self,
        had_original_index: impl Fn(entity::Raw) -> bool,
    ) -> Result<(), ValidationError> {
        #[cfg(debug_assertions)]
        {
            let mut inverse = HashMap::with_capacity(self.index.len());
            for (original, &target) in self.index.iter().enumerate() {
                let original = entity::Raw(original.small_int());

                if target.0.small_int::<usize>() >= self.index.len() {
                    return Err(ValidationError::OutOfRange(OutOfRangeError {
                        position: original,
                        target,
                    }));
                }

                if had_original_index(original) {
                    match inverse.entry(original) {
                        hash_map::Entry::Occupied(entry) => {
                            return Err(ValidationError::Duplicate(DuplicateError {
                                positions: [original, *entry.get()],
                                target,
                            }));
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
        referrer.visit_each(archetype, &mut |r| self.update(r))
    }

    /// Updates an entity reference with the permutation.
    pub(crate) fn update(&self, entity: &mut entity::Raw) {
        let original = *entity;
        let new =
            self.index.get(original.0.small_int::<usize>()).expect("Permutation out of bounds");
        *entity = *new;
    }
}

/// An error returned when the permutation is invalid.
#[derive(Debug, PartialEq)]
pub(crate) enum ValidationError {
    /// An entity was mapped to a position that is out of range.
    OutOfRange(OutOfRangeError),
    /// Multiple original indices point to the same target.
    Duplicate(DuplicateError),
}

/// An entity was mapped to a position that is out of range.
#[derive(Debug, PartialEq)]
pub(crate) struct OutOfRangeError {
    /// The original index.
    position: entity::Raw,
    /// The out-of-range index.
    target:   entity::Raw,
}

/// Multiple original indices point to the same target.
#[derive(Debug, PartialEq)]
pub(crate) struct DuplicateError {
    /// The two original indices that alias the same target.
    positions: [entity::Raw; 2],
    /// The target index.
    target:    entity::Raw,
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::collections::HashMap;

    use super::Permutation;
    use crate::entity::{self, RefId};
    use crate::test_util::TestArch;

    #[test]
    fn test_from_comparator() {
        let mut weight: HashMap<entity::Raw, i32> = HashMap::new();
        weight.insert(entity::Raw(1), 5);
        weight.insert(entity::Raw(3), 3);
        weight.insert(entity::Raw(5), 4);

        let permutation =
            Permutation::from_comparator(weight.keys().copied(), |entity1, entity2| {
                let weight1 = weight.get(&entity1).expect("Undefined key given");
                let weight2 = weight.get(&entity2).expect("Undefined key given");
                weight1.cmp(weight2)
            });

        for (original, target) in [(1, 2), (3, 0), (5, 1)] {
            let mut entity = crate::entity::Entity::<TestArch>::allocate_new(entity::Raw(original));
            permutation.update_referrer(TypeId::of::<TestArch>(), &mut entity);
            assert_eq!(entity.id().0 .0, target);
        }
    }
}
