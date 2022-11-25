//! Utilities for dynamic dispatch related to components.

use std::any::{self, Any};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::{cmp, hash};

use super::Discrim;
use crate::util::DbgTypeId;
use crate::{comp, storage, Archetype};

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

pub trait Iter<A: Archetype> {
    fn get_simple<C: comp::Simple<A>>(&mut self) -> Option<&mut C>;

    fn fill_simple(
        &mut self,
        entity: A::RawEntity,
        ty: DbgTypeId,
        storage: &mut dyn storage::AnySimple<A>,
    );

    type IterIsotopeType<'t>: Iterator<Item = DbgTypeId> + 't
    where
        Self: 't;
    type IterIsotopeValue<'t>: SimpleIter<'t, A>
    where
        Self: 't;
    fn iter_isotope(&mut self) -> (Self::IterIsotopeType<'_>, Self::IterIsotopeValue<'_>);
}

pub trait SimpleIter<'t, A: Archetype>: 't {
    fn next<C: comp::Simple<A>>(&mut self) -> Option<C>;
}
pub trait IsotopeIter<'t, A: Archetype>: 't {
    fn next<C: comp::Isotope<A>>(&mut self) -> Option<C>;
}

/// Dependency list.
///
/// Items are tuples of (DbgTypeIdOf::<C>(), C::INIT_STRATEGY).
pub type DepList<A> = Vec<(DbgTypeId, comp::SimpleInitStrategy<A>)>;

/// Describes how to instantiate a component based on other component types.
pub struct SimpleIniter<A: Archetype> {
    /// The component function.
    pub f: &'static dyn SimpleInitFn<A>,
}

/// A function used for [`comp::SimpleInitStrategy::Auto`].
///
/// This trait is blanket-implemented for all functions that take up to 32 simple component
/// parameters and output the component value.
pub trait SimpleInitFn<A: Archetype>: Send + Sync + 'static {
    /// Calls the underlying function, extracting the arguments.
    fn populate(&self, map: &mut Map<A>);

    /// Returns the component types required by this function.
    fn deps(&self) -> DepList<A>;
}

macro_rules! impl_simple_init_fn {
    ($($deps:ident),* $(,)?) => {
        impl<
            A: Archetype, C: comp::Simple<A>,
            $($deps: comp::Simple<A>,)*
        > SimpleInitFn<A> for fn(
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

macro_rules! impl_simple_init_fn_accumulate {
    () => {
        impl_simple_init_fn!();
    };
    ($first:ident $(, $rest:ident)* $(,)?) => {
        impl_simple_init_fn_accumulate!($($rest),*);
        impl_simple_init_fn!($first $(, $rest)*);
    }
}
impl_simple_init_fn_accumulate!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23, P24, P25, P26, P27, P28, P29, P30, P31, P32,
);

/// Describes how to instantiate a component based on other component types.
pub struct IsotopeIniter<A: Archetype> {
    /// The component function.
    pub f: &'static dyn IsotopeInitFn<A>,
}

/// A function used for [`comp::IsotopeInitStrategy::Auto`].
///
/// This trait is blanket-implemented for all functions that take up to 32 isotope component
/// parameters and output an iterator of (discriminant, component) tuples.
pub trait IsotopeInitFn<A: Archetype>: Send + Sync + 'static {
    /// Calls the underlying function, extracting the arguments.
    fn populate(&self, map: &mut Map<A>);

    /// Returns the component types required by this function.
    fn deps(&self) -> DepList<A>;
}

macro_rules! impl_isotope_init_fn {
    ($($deps:ident),* $(,)?) => {
        impl<
            A: Archetype, C: comp::Isotope<A>,
            I: IntoIterator<Item = (C::Discrim, C)> + 'static,
            $($deps: comp::Simple<A>,)*
        > IsotopeInitFn<A> for fn(
            $(&$deps,)*
        ) -> I {
            fn populate(&self, map: &mut Map<A>) {
                let iter = (self)(
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
                for (discrim, value) in iter {
                    map.insert_isotope(discrim, value);
                }
            }

            fn deps(&self) -> Vec<(DbgTypeId, comp::SimpleInitStrategy<A>)> {
                vec![
                    $((DbgTypeId::of::<$deps>(), <$deps as comp::Simple<A>>::INIT_STRATEGY),)*
                ]
            }
        }
    }
}

macro_rules! impl_isotope_init_fn_accumulate {
    () => {
        impl_isotope_init_fn!();
    };
    ($first:ident $(, $rest:ident)* $(,)?) => {
        impl_isotope_init_fn_accumulate!($($rest),*);
        impl_isotope_init_fn!($first $(, $rest)*);
    }
}
impl_isotope_init_fn_accumulate!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23, P24, P25, P26, P27, P28, P29, P30, P31, P32,
);

#[cfg(test)]
mod tests;
