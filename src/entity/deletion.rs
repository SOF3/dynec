//! Manages entity deletion logic.

use std::any::TypeId;
use std::collections::HashMap;

use bitvec::prelude::BitVec;

use super::Raw;
use crate::util::DbgTypeId;
use crate::Archetype;

/// Stores whether an entity is marked for deletion.
#[crate::global(dynec_as(crate))]
#[derive(Default)]
pub struct Flags {
    flags: HashMap<DbgTypeId, BitVec>,
}

impl Flags {
    /// Checks whether an entity is marked for deletion.
    pub fn get<A: Archetype>(&self, id: A::RawEntity) -> bool {
        match self.flags.get(&TypeId::of::<A>()) {
            Some(flags) => flags.get(id.to_primitive()).as_deref().copied().unwrap_or(false),
            None => false,
        }
    }

    /// Marks or unmarks an entity for deletion.
    pub fn set<A: Archetype>(&mut self, id: A::RawEntity, value: bool) {
        let id = id.to_primitive();

        let vec = self.flags.entry(DbgTypeId::of::<A>()).or_default();
        if vec.len() <= id {
            vec.resize(id + 1, false);
        }

        vec.set(id, value);
    }
}
