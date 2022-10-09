//! Utilities for dynamic dispatch related to components.

use std::any::{self, Any};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::{cmp, hash};

use super::Discrim;
use crate::util::DbgTypeId;
use crate::{comp, Archetype};

/// Identifies a generic simple or discriminated isotope component type.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Identifier {
    pub(crate) id:      DbgTypeId,
    pub(crate) discrim: Option<usize>,
}

impl Identifier {
    pub(crate) fn simple<A: Archetype, C: comp::Simple<A>>() -> Self {
        Identifier { id: DbgTypeId::of::<C>(), discrim: None }
    }

    pub(crate) fn isotope<A: Archetype, C: comp::Isotope<A>>(discrim: C::Discrim) -> Self {
        Identifier { id: DbgTypeId::of::<C>(), discrim: Some(discrim.into_usize()) }
    }
}

impl PartialEq for Identifier {
    fn eq(&self, other: &Self) -> bool { self.id == other.id && self.discrim == other.discrim }
}

impl Eq for Identifier {}

impl PartialOrd for Identifier {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> { Some(self.cmp(other)) }
}

impl Ord for Identifier {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.id.cmp(&other.id).then_with(|| self.discrim.cmp(&other.discrim))
    }
}

impl hash::Hash for Identifier {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.discrim.hash(state)
    }
}

/// A generic TypeMap of owned simple and isotope components.
///
/// This type is only used in parameter passing, not in the actual storage.
pub struct Map<A: Archetype> {
    map: BTreeMap<Identifier, Box<dyn Any + Send + Sync>>,

    _ph: PhantomData<A>,
}

impl<A: Archetype> Default for Map<A> {
    fn default() -> Self { Map { map: BTreeMap::default(), _ph: PhantomData } }
}

impl<A: Archetype> Map<A> {
    /// Inserts a simple component into the map.
    pub fn insert_simple<C: comp::Simple<A>>(&mut self, comp: C) {
        let prev = self.map.insert(Identifier::simple::<A, C>(), Box::new(comp));
        if prev.is_some() {
            panic!("Cannot insert the same simple component into the same comp::Map twice");
        }
    }

    /// Gets a simple component from the map.
    pub(crate) fn get_simple<C: comp::Simple<A>>(&self) -> Option<&C> {
        self.map.get(&Identifier::simple::<A, C>()).and_then(|c| c.downcast_ref())
    }

    /// Gets a simple component from the map.
    pub(crate) fn remove_simple<C: comp::Simple<A>>(&mut self) -> Option<C> {
        let comp = self.map.remove(&Identifier::simple::<A, C>())?;
        let comp = comp.downcast::<C>().expect("TypeId mismatch");
        Some(*comp)
    }

    /// Inserts an isotope component into the map.
    pub fn insert_isotope<C: comp::Isotope<A>>(&mut self, discrim: C::Discrim, comp: C) {
        let prev = self.map.insert(Identifier::isotope::<A, C>(discrim), Box::new(comp));
        if prev.is_some() {
            panic!(
                "Cannot insert the same isotope component with the same discriminant into the \
                 same comp::Map twice"
            );
        }
    }

    /// Drops this map, returning an iterator of all isotope components.
    ///
    /// This should be changed to `drain_filter` when it is stable.
    pub(crate) fn into_isotopes(
        self,
    ) -> impl Iterator<Item = (Identifier, Box<dyn Any + Send + Sync>)> {
        self.map.into_iter().filter(|(id, _)| id.discrim.is_some())
    }

    /// Returns the number of components in the map.
    pub fn len(&self) -> usize { self.map.len() }

    /// Returns true if the map contains no components.
    pub fn is_empty(&self) -> bool { self.map.is_empty() }
}

/// Describes how to instantiate a component based on other component types.
pub struct AutoIniter<A: Archetype> {
    /// The component function.
    pub f: &'static dyn AutoInitFn<A>,
}

/// A function used for [`comp::SimpleInitStrategy::Auto`].
///
/// This trait is blanket-implemented for all functions that take up to 32 simple component
/// parameters.
pub trait AutoInitFn<A: Archetype>: Send + Sync + 'static {
    /// Calls the underlying function, extracting the arguments.
    fn populate(&self, map: &mut Map<A>);

    /// Returns the component types required by this function.
    fn deps(&self) -> Vec<(DbgTypeId, comp::SimpleInitStrategy<A>)>;
}

macro_rules! impl_auto_init_fn {
    ($($deps:ident),* $(,)?) => {
        impl<
            A: Archetype, C: comp::Simple<A>,
            $($deps: comp::Simple<A>,)*
        > AutoInitFn<A> for fn(
            $(&$deps,)*
        ) -> C {
            fn populate(&self, map: &mut Map<A>) {
                let populate = (self)(
                    $(
                        match map.get_simple::<$deps>() {
                            Some(comp) => comp,
                            None => panic!(
                                "Cannot create an entity of type `{}` without explicitly passing a component of type `{}`, which is required for `{}`",
                                any::type_name::<A>(),
                                any::type_name::<$deps>(),
                                any::type_name::<C>(),
                            ),
                        },
                    )*
                );
                map.insert_simple(populate);
            }

            fn deps(&self) -> Vec<(DbgTypeId, comp::SimpleInitStrategy<A>)> {
                vec![
                    $((DbgTypeId::of::<$deps>(), <$deps as comp::Simple<A>>::INIT_STRATEGY),)*
                ]
            }
        }
    }
}

macro_rules! impl_auto_init_fn_accumulate {
    () => {
        impl_auto_init_fn!();
    };
    ($first:ident $(, $rest:ident)* $(,)?) => {
        impl_auto_init_fn_accumulate!($($rest),*);
        impl_auto_init_fn!($first $(, $rest)*);
    }
}
impl_auto_init_fn_accumulate!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23, P24, P25, P26, P27, P28, P29, P30, P31, P32,
);

#[cfg(test)]
mod tests;
