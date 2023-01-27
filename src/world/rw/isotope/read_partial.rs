use std::fmt;
use std::marker::PhantomData;

use crate::comp::{self, discrim};
use crate::{system, world, Archetype};

impl world::Components {
    /// Creates a read-only, shared accessor to specific discriminants of the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is exclusively accessing any of the requested discriminants.
    pub fn read_partial_isotope_storage<'t, A, C, DiscrimSet>(
        &'t self,
        discrims: &'t DiscrimSet,
    ) -> impl system::ReadIsotope<A, C, DiscrimSet::Key> + 't
    where
        A: Archetype,
        C: comp::Isotope<A>,
        DiscrimSet: discrim::Set<C::Discrim>,
    {
        let storage_map = self.isotope_storage_map::<A, C>();

        let storages: DiscrimSet::Mapped<Option<_>> = {
            // note: lock contention may occur here if another thread is requesting write access to
            // storages of other discriminants.
            let map = storage_map.map.read();

            discrims.map(|discrim| {
                Some(super::own_read_isotope_storage::<A, C>(discrim, map.map().get(&discrim)?))
            })
        };

        super::IsotopeAccessor {
            storages,
            processor: Proc::<A, C, _>(PhantomData),
            _ph: PhantomData,
        }
    }
}

struct Proc<A, C, S>(PhantomData<(A, C, S)>);
impl<A, C, S> super::StorageMapProcessorRef for Proc<A, C, S> {
    type Input = Option<S>;
    type Output = S;

    fn process<'t, D: fmt::Debug, F: FnOnce() -> D>(
        &self,
        input: Option<&'t Option<S>>,
        key: F,
    ) -> Option<&'t S> {
        match input {
            Some(Some(storage)) => Some(storage), // already initialized
            Some(None) => None,                   // valid discriminant, but not yet initialized
            None => super::panic_invalid_key::<A, C>(key()),
        }
    }

    fn admit(input: &Option<S>) -> Option<&S> { input.as_ref() }
}
