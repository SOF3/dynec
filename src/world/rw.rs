//! Component reading and writing.

use std::any::type_name;
use std::collections::HashMap;
use std::marker::PhantomData;

use super::typed;
use crate::util::DbgTypeId;
use crate::{entity, storage, system, Archetype};

pub(crate) mod isotope;
pub(crate) mod simple;

/// Stores the component states in a world.
pub struct Components {
    pub(crate) archetypes: HashMap<DbgTypeId, Box<dyn typed::AnyTyped>>,
}

impl Components {
    /// Creates a dummy, empty component store used for testing.
    pub fn empty() -> Self { Self { archetypes: HashMap::new() } }

    /// Fetches the [`Typed`](typed::Typed) for the requested archetype.
    pub(crate) fn archetype<A: Archetype>(&self) -> &typed::Typed<A> {
        match self.archetypes.get(&DbgTypeId::of::<A>()) {
            Some(typed) => typed.as_any().downcast_ref().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                type_name::<A>()
            ),
        }
    }

    /// Fetches the [`Typed`](typed::Typed) for the requested archetype.
    pub(crate) fn archetype_mut<A: Archetype>(&mut self) -> &mut typed::Typed<A> {
        match self.archetypes.get_mut(&DbgTypeId::of::<A>()) {
            Some(typed) => typed.as_any_mut().downcast_mut().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                type_name::<A>()
            ),
        }
    }
}

struct PartitionAccessor<A: Archetype, C, S: storage::Partition<A::RawEntity, C>> {
    storage: S,
    _ph:     PhantomData<(A, C)>,
}
impl<A, C, StorageParT> system::Mut<A, C> for PartitionAccessor<A, C, StorageParT>
where
    A: Archetype,
    C: 'static,
    StorageParT: storage::Partition<A::RawEntity, C>,
{
    fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C> {
        self.storage.get_mut(entity.id())
    }

    type IterMut<'u> = impl Iterator<Item = (entity::TempRef<'u, A>, &'u mut C)> + 'u where Self: 'u;
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.storage.iter_mut().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }

    type SplitEntitiesAt<'u> = impl system::Mut<A, C> + 'u where Self: 'u;
    fn split_entities_at<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
    ) -> (Self::SplitEntitiesAt<'_>, Self::SplitEntitiesAt<'_>) {
        let (left, right) = self.storage.partition_at(entity.id());
        (
            PartitionAccessor { storage: left, _ph: PhantomData },
            PartitionAccessor { storage: right, _ph: PhantomData },
        )
    }
}

#[cfg(test)]
static_assertions::assert_impl_all!(Components: Send, Sync);
