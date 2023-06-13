use std::marker::PhantomData;

use crate::comp::{self, discrim};
use crate::entity::ealloc;
use crate::world::rw::isotope;
use crate::{system, world, Archetype};

impl world::Components {
    pub fn write_partial_isotope_storage<'t, A, C, DiscrimSet>(
        &'t self,
        discrims: &'t DiscrimSet,
        snapshot: ealloc::Snapshot<A::RawEntity>,
    ) -> impl system::WriteIsotope<A, C, DiscrimSet::Key> + 't
    where
        A: Archetype,
        C: comp::Isotope<A>,
        DiscrimSet: discrim::Set<C::Discrim>,
    {
        isotope::Base { getter: Getter { _ph: PhantomData } }
    }
}

struct Getter<A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    _ph: PhantomData<(A, C)>,
}
