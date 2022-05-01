use std::any::TypeId;
use std::cmp;
use std::num::NonZeroU32;

use xias::Xias;

use crate::entity;

/// A permutation of entities.
pub struct Permutation {
    pub(crate) index: Vec<Option<Permuted>>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Original {
    original: entity::Raw,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Permuted {
    permuted: entity::Raw,
}

impl Permutation {
    /// Creates a new permutation with a comparator on raw entity positions.
    pub(crate) fn from_comparator(
        positions: impl IntoIterator<Item = entity::Raw>,
        mut comparator: impl FnMut(entity::Raw, entity::Raw) -> cmp::Ordering,
    ) -> Self {
        // If `inverse[k] == v`, entity at position `v` is moved to position `k + 1`.
        let mut inverse: Vec<Original> =
            positions.into_iter().map(|original| Original { original }).collect();
        if inverse.is_empty() {
            return Self { index: vec![] };
        }

        inverse.sort_unstable_by(|&a, &b| comparator(a.original, b.original));

        // compute the largest original index + 1 as the map size
        let index_len = inverse
            .iter()
            .copied()
            .max()
            .expect("inverse is nonempty")
            .original
            .0
            .get()
            .small_int::<usize>()
            + 1;

        let mut index: Vec<Option<Permuted>> = vec![None; index_len];
        for (permuted_minus_one, original) in inverse.iter().enumerate() {
            let entry = index
                .get_mut(original.original.0.get().small_int::<usize>())
                .expect("index_len > all inverse values");

            let permuted = (permuted_minus_one + 1).small_int();
            let permuted = NonZeroU32::new(permuted).expect("already added one");
            let permuted = Permuted { permuted: entity::Raw(permuted) };

            *entry = Some(permuted);
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

    /// Updates an entity referrer with the permutation.
    pub fn update_referrer(&self, archetype: TypeId, referrer: &mut impl entity::Referrer) {
        referrer.visit_each(archetype, &mut |r| self.update(r))
    }

    /// Updates an entity reference with the permutation.
    pub(crate) fn update(&self, entity: &mut entity::Raw) {
        let original = Original { original: *entity };
        let permuted = self.get(original);
        *entity = permuted.permuted;
    }

    pub(crate) fn get(&self, original: Original) -> Permuted {
        match self.index.get(original.original.0.get().small_int::<usize>()) {
            Some(&Some(permuted)) => permuted,
            _ => panic!("Attempt to permute nonexistent entity"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::collections::HashMap;

    use super::Permutation;
    use crate::entity::RefId;
    use crate::{entity, TestArch};

    #[test]
    fn test_from_comparator() {
        let mut weight: HashMap<entity::Raw, i32> = HashMap::new();
        weight.insert(entity::Raw::testing(1), 5);
        weight.insert(entity::Raw::testing(3), 3);
        weight.insert(entity::Raw::testing(5), 4);

        let permutation =
            Permutation::from_comparator(weight.keys().copied(), |entity1, entity2| {
                let weight1 = weight.get(&entity1).expect("Undefined key given");
                let weight2 = weight.get(&entity2).expect("Undefined key given");
                weight1.cmp(weight2)
            });

        for (original, target) in [(1, 3), (3, 1), (5, 2)] {
            let mut entity =
                crate::entity::Entity::<TestArch>::new_allocated(entity::Raw::testing(original));
            permutation.update_referrer(TypeId::of::<TestArch>(), &mut entity);
            assert_eq!(entity.id().0 .0.get(), target);
        }
    }
}
