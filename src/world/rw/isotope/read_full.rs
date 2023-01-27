use std::fmt;
use std::marker::PhantomData;

use crate::comp::{self, Discrim};
use crate::{system, world, Archetype};

impl world::Components {
    /// Creates a read-only, shared accessor to all discriminants of the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is exclusively accessing any discriminants of the isotope component.
    pub fn read_full_isotope_storage<A, C>(&self) -> impl system::ReadIsotope<A, C> + '_
    where
        A: Archetype,
        C: comp::Isotope<A>,
    {
        let storage_map = self.isotope_storage_map::<A, C>();

        let storages: <C::Discrim as Discrim>::FullMap<_> = {
            // note: lock contention may occur here if another thread is requesting write access to
            // storages of other discriminants.
            let map = storage_map.map.read();

            map.map()
                .iter()
                .map(|(&discrim, storage)| {
                    (discrim, super::own_read_isotope_storage::<A, C>(discrim, storage))
                })
                .collect()
        };

        super::IsotopeAccessor { storages, processor: Proc(PhantomData), _ph: PhantomData }
    }
}

struct Proc<S>(PhantomData<S>);
impl<S> super::StorageMapProcessorRef for Proc<S> {
    type Input = S;
    type Output = S;
    fn process<'t, D: fmt::Debug, F: FnOnce() -> D>(
        &self,
        input: Option<&'t S>,
        _: F,
    ) -> Option<&'t S> {
        input
    }
    fn admit(input: &S) -> Option<&S> { Some(input) }
}
