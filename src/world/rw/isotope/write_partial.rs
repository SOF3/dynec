use std::marker::PhantomData;
use std::{fmt, ops};

use crate::comp::{self, discrim};
use crate::entity::ealloc;
use crate::{system, world, Archetype};

impl world::Components {
    /// Creates a writable, exclusive accessor to the given archetyped isotope component,
    /// initializing new discriminants if not previously created.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems
    /// - if another thread is accessing the same archetyped component.
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
        let storage_map = self.isotope_storage_map::<A, C>();

        let storages = {
            // note: lock contention may occur here if another thread is requesting write access to
            // storages of other discriminants.
            let mut map = storage_map.map.write();

            discrims.map(|discrim| {
                let storage = map.get_or_create(discrim, snapshot.iter_allocated_chunks());
                super::own_write_isotope_storage::<A, C>(discrim, storage)
            })
        };

        super::IsotopeAccessor { storages, processor: Proc(PhantomData), _ph: PhantomData }
    }
}

struct Proc<A, C, S>(PhantomData<(A, C, S)>);
impl<A, C, S> super::StorageMapProcessorRef for Proc<A, C, S> {
    type Input = S;
    type Output = S;

    fn process<'t, D: fmt::Debug, F: FnOnce() -> D>(
        &self,
        input: Option<&'t S>,
        key: F,
    ) -> Option<&'t S> {
        match input {
            Some(input) => Some(input),
            None => super::panic_invalid_key::<A, C>(key()),
        }
    }

    fn admit(input: &Self::Input) -> Option<&Self::Output> { Some(input) }
}
impl<A, C, S, M> super::MutStorageAccessor<A, C, S, M> for Proc<A, C, S>
where
    A: Archetype,
    C: comp::Isotope<A>,
    S: ops::DerefMut<Target = C::Storage>,
    M: discrim::Mapped<Discrim = C::Discrim, Value = S>,
{
    fn get_storage<'t>(&mut self, key: M::Key, storages: &'t mut M) -> &'t mut C::Storage
    where
        S: 't,
    {
        match storages.get_mut_by(key).map(|s| &mut **s) {
            Some(storage) => storage,
            None => panic!(
                "Cannot access isotope indexed by {key:?} because it is not in the list of \
                 requested discriminants",
            ),
        }
    }

    fn get_storage_multi<'u, const N: usize>(
        &mut self,
        keys: [M::Key; N],
        storages: &'u mut M,
    ) -> [&'u mut C::Storage; N]
    where
        S: 'u,
    {
        storages.get_mut_array_by(
            keys,
            |storage| -> &mut C::Storage { &mut *storage },
            |key| {
                panic!(
                    "Cannot access isotope indexed by {key:?} because it is not in the list of \
                     requested discriminants",
                )
            },
        )
    }
}
