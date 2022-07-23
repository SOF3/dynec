//! Tracks the number of times an entity ID is allocated,
//! used for distinguishment of dangling weak references.

/// The number of times the same entry has been used for allocating an entity.
/// This type is fully ordered, where a greater generation implies newer version.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Generation(u32);

/// Stores generations of entities.
#[derive(Default)]
pub struct Store {
    vec: Vec<Generation>,
}

impl Store {
    pub fn next(&mut self, id: usize) -> Generation {
        if self.vec.len() <= id {
            self.vec.resize(id + 1, Generation::default());
        }

        let generation = self.vec.get_mut(id).expect("just resized");
        generation.0 = generation.0.wrapping_add(1);
        *generation
    }

    pub fn get(&self, id: usize) -> Generation { self.vec.get(id).copied().unwrap_or_default() }
}
