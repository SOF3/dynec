use std::collections::BTreeMap;

use super::Storage;
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

    fn get(&self, id: Self::RawEntity) -> Option<&C> { self.data.get(&id) }

    fn get_mut(&mut self, id: Self::RawEntity) -> Option<&mut C> { self.data.get_mut(&id) }

    fn set(&mut self, id: Self::RawEntity, new: Option<C>) -> Option<C> {
        match new {
            Some(new) => self.data.insert(id, new),
            None => self.data.remove(&id),
        }
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (Self::RawEntity, &C)> + '_> {
        Box::new(self.data.iter().map(|(&k, v)| (k, v)))
    }

    fn iter_mut(&mut self) -> Box<dyn Iterator<Item = (Self::RawEntity, &mut C)> + '_> {
        Box::new(self.data.iter_mut().map(|(&k, v)| (k, v)))
    }
}
