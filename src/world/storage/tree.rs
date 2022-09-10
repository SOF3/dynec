use std::collections::BTreeMap;
use std::slice;

use super::{ChunkMut, ChunkRef, Storage};
use crate::entity;

/// A storage based on [`BTreeMap`].
pub struct Tree<E: entity::Raw, C> {
    data: BTreeMap<E, C>,
}

impl<E: entity::Raw, C> Default for Tree<E, C> {
    fn default() -> Self { Self { data: BTreeMap::new() } }
}

impl<E: entity::Raw, C: Send + Sync + 'static> Storage for Tree<E, C> {
    type RawEntity = E;
    type Comp = C;

    type Iter<'t> = impl Iterator<Item = (Self::RawEntity, &'t Self::Comp)> + 't;
    type IterChunks<'t> = impl Iterator<Item = ChunkRef<'t, Self>> + 't;
    type IterMut<'t> = impl Iterator<Item = (Self::RawEntity, &'t mut Self::Comp)> + 't;
    type IterChunksMut<'t> = impl Iterator<Item = ChunkMut<'t, Self>> + 't;

    fn get(&self, id: Self::RawEntity) -> Option<&C> { self.data.get(&id) }

    fn get_mut(&mut self, id: Self::RawEntity) -> Option<&mut C> { self.data.get_mut(&id) }

    fn set(&mut self, id: Self::RawEntity, new: Option<C>) -> Option<C> {
        match new {
            Some(new) => self.data.insert(id, new),
            None => self.data.remove(&id),
        }
    }

    fn cardinality(&self) -> usize { self.data.len() }

    fn iter(&self) -> Self::Iter<'_> { self.data.iter().map(|(&k, v)| (k, v)) }

    fn iter_chunks(&self) -> Self::IterChunks<'_> {
        self.iter().map(|(entity, item)| ChunkRef { slice: slice::from_ref(item), start: entity })
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        Box::new(self.data.iter_mut().map(|(&k, v)| (k, v)))
    }

    fn iter_chunks_mut(&mut self) -> Self::IterChunksMut<'_> {
        self.iter_mut()
            .map(|(entity, item)| ChunkMut { slice: slice::from_mut(item), start: entity })
    }
}
