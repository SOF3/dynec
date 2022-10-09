//! Manages entity deletion logic.

use crate::{comp, storage, Archetype};

/// The flag exists as a component of an entity
/// if and only if the entity was marked for deletion.
pub(crate) struct Flag(pub(crate) ());

impl super::Referrer for Flag {
    fn visit_type(arg: &mut super::referrer::VisitTypeArg) { arg.mark::<Self>(); }
    fn visit_mut<V: super::referrer::VisitMutArg>(&mut self, _: &mut V) {}
}

impl<A: Archetype> comp::Simple<A> for Flag {
    type Storage = storage::Vec<A::RawEntity, Self>;

    const PRESENCE: comp::SimplePresence = comp::SimplePresence::Optional;
    const INIT_STRATEGY: comp::SimpleInitStrategy<A> = comp::SimpleInitStrategy::None;
    const IS_FINALIZER: bool = false;
}
