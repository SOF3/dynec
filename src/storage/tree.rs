use std::cell::SyncUnsafeCell;
use std::collections::BTreeMap;
use std::slice;

use super::{ChunkMut, ChunkRef, Storage};
use crate::entity;

/// A storage based on [`BTreeMap`].
pub struct Tree<E: entity::Raw, C> {
    // `SyncUnsafeCell<C>` here must be treaeted as a normal `C`
    // unless the whole storage is mutably locked,
    // which means the current function exclusively manages this map.
    // `&Tree` must not be used to access the cells mutably.
    data: BTreeMap<E, SyncUnsafeCell<C>>,
}

impl<E: entity::Raw, C> Default for Tree<E, C> {
    fn default() -> Self { Self { data: BTreeMap::new() } }
}

// Safety: the backend of `get`/`get_mut` is a BTreeSet,
// which is defined to be injective
// assuming correct implementation of Eq + Ord.
unsafe impl<E: entity::Raw, C: Send + Sync + 'static> Storage for Tree<E, C> {
    type RawEntity = E;
    type Comp = C;

    fn get(&self, id: Self::RawEntity) -> Option<&C> {
        self.data.get(&id).map(|cell| unsafe {
            // Safety: `&self` implies that nobody else can mutate the values.
            &*cell.get()
        })
    }

    fn get_mut(&mut self, id: Self::RawEntity) -> Option<&mut C> {
        self.data.get_mut(&id).map(|cell| cell.get_mut())
    }

    fn set(&mut self, id: Self::RawEntity, new: Option<C>) -> Option<C> {
        match new {
            Some(new) => self.data.insert(id, SyncUnsafeCell::new(new)),
            None => self.data.remove(&id),
        }
        .map(|cell| cell.into_inner())
    }

    fn cardinality(&self) -> usize { self.data.len() }

    type Iter<'t> = impl Iterator<Item = (Self::RawEntity, &'t Self::Comp)> + 't;
    fn iter(&self) -> Self::Iter<'_> {
        self.data.iter().map(|(&entity, cell)| {
            (entity, unsafe {
                // Safety: `&self` implies that nobody else can mutate the values.
                &*cell.get()
            })
        })
    }

    type IterChunks<'t> = impl Iterator<Item = ChunkRef<'t, Self>> + 't;
    fn iter_chunks(&self) -> Self::IterChunks<'_> {
        self.iter().map(|(entity, item)| ChunkRef { slice: slice::from_ref(item), start: entity })
    }

    type IterMut<'t> = impl Iterator<Item = (Self::RawEntity, &'t mut Self::Comp)> + 't;
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        Box::new(self.data.iter_mut().map(|(&entity, cell)| (entity, cell.get_mut())))
    }

    type IterChunksMut<'t> = impl Iterator<Item = ChunkMut<'t, Self>> + 't;
    fn iter_chunks_mut(&mut self) -> Self::IterChunksMut<'_> {
        self.iter_mut()
            .map(|(entity, item)| ChunkMut { slice: slice::from_mut(item), start: entity })
    }

    type Partition<'t> = StoragePartition<'t, E, C>;
    fn as_partition(&mut self) -> Self::Partition<'_> {
        StoragePartition { data: &self.data, lower_bound: None, upper_bound: None }
    }
}

/// Return value of [`Tree::partition_at`].
pub struct StoragePartition<'t, E: entity::Raw, C> {
    data:        &'t BTreeMap<E, SyncUnsafeCell<C>>,
    lower_bound: Option<E>,
    upper_bound: Option<E>,
}

impl<'t, E: entity::Raw, C> StoragePartition<'t, E, C> {
    fn assert_bounds(&self, entity: E) {
        if let Some(bound) = self.lower_bound {
            assert!(entity >= bound, "Entity {entity:?} is not in the partition {bound:?}..");
        }
        if let Some(bound) = self.upper_bound {
            assert!(entity < bound, "Entity {entity:?} is not in the partition ..{bound:?}");
        }
    }
}

impl<'t, E: entity::Raw, C: Send + Sync + 'static> super::Partition<'t, E, C>
    for StoragePartition<'t, E, C>
{
    type ByRef<'u> = StoragePartition<'u, E, C> where Self: 'u;
    fn by_ref(&mut self) -> Self::ByRef<'_> {
        StoragePartition {
            data:        self.data,
            lower_bound: self.lower_bound,
            upper_bound: self.upper_bound,
        }
    }

    fn get_mut(&mut self, entity: E) -> Option<&mut C> {
        self.assert_bounds(entity);

        let cell = self.data.get(&entity)?;
        unsafe {
            // Safety: StoragePartition locks all keys under `self.cmp` exclusively,
            // and our key is under `self.cmp`.
            // We already have `&mut self`, so no other threads are accessing this range.
            Some(&mut *cell.get())
        }
    }

    type IterMut = impl Iterator<Item = (E, &'t mut C)>;
    fn iter_mut(self) -> Self::IterMut {
        let iter = match (self.lower_bound, self.upper_bound) {
            (Some(lower), Some(upper)) => Box::new(self.data.range(lower..upper))
                as Box<dyn Iterator<Item = (&E, &SyncUnsafeCell<C>)>>,
            (Some(lower), None) => Box::new(self.data.range(lower..)),
            (None, Some(upper)) => Box::new(self.data.range(..upper)),
            (None, None) => Box::new(self.data.iter()),
        };

        iter.map(|(entity, cell)| unsafe {
            // Safety: StoragePartition locks all keys under `self.cmp` exclusively,
            // and the key is within the valid range due to .range().
            // We already have `&mut self`, so no other threads are accessing this range.

            (*entity, &mut *cell.get())
        })
    }

    fn partition_at(self, entity: E) -> (Self, Self) {
        self.assert_bounds(entity);

        // Safety: `entity` is between lower_bound and upper_bound,
        // so the resultant bound will be non-overlapping.
        // We already have `&mut self`, so this range cannot be used until the partitions are
        // dropped.
        (
            Self {
                data:        self.data,
                lower_bound: self.lower_bound,
                upper_bound: Some(entity),
            },
            Self {
                data:        self.data,
                lower_bound: Some(entity),
                upper_bound: self.upper_bound,
            },
        )
    }
}

#[cfg(test)]
super::tests::test_storage!(NON_CHUNKED Tree<std::num::NonZeroU32, i64>);
