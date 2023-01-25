//! Utilities for dynamic dispatch related to components.

use std::any::{self, Any};
use std::collections::HashMap;
use std::marker::PhantomData;

use parking_lot::lock_api::ArcRwLockWriteGuard;

use crate::storage::simple::AnySimpleStorage;
use crate::util::DbgTypeId;
use crate::{comp, storage, Archetype};

/// A generic TypeMap of owned simple and isotope components.
///
/// This type is only used in parameter passing, not in the actual storage.
pub struct Map<A: Archetype> {
    simple:  HashMap<DbgTypeId, Box<dyn Any + Send + Sync>>,
    isotope: HashMap<DbgTypeId, Box<dyn Any + Send + Sync>>,

    _ph: PhantomData<A>,
}

impl<A: Archetype> Default for Map<A> {
    fn default() -> Self {
        Map { simple: HashMap::new(), isotope: HashMap::new(), _ph: PhantomData }
    }
}

pub(crate) type IsotopeMap<A, C> = Vec<(<C as comp::Isotope<A>>::Discrim, C)>;

impl<A: Archetype> Map<A> {
    /// Inserts a simple component into the map.
    pub fn insert_simple<C: comp::Simple<A>>(&mut self, comp: C) {
        let prev = self.simple.insert(DbgTypeId::of::<C>(), Box::new(comp));
        if prev.is_some() {
            panic!("Cannot insert the same simple component into the same comp::Map twice");
        }
    }

    pub(crate) fn remove_simple<C: comp::Simple<A>>(&mut self) -> Option<C> {
        Some(*self.simple.remove(&DbgTypeId::of::<C>())?.downcast().expect("TypeId mismatch"))
    }

    /// Inserts an isotope component into the map.
    pub fn insert_isotope<C: comp::Isotope<A>>(&mut self, discrim: C::Discrim, comp: C) {
        let entry = self.isotope.entry(DbgTypeId::of::<C>());
        entry
            .or_insert_with(|| Box::<IsotopeMap<A, C>>::default())
            .downcast_mut::<IsotopeMap<A, C>>()
            .expect("TypeId mismatch")
            .push((discrim, comp));
    }

    pub(crate) fn remove_isotope<C: comp::Isotope<A>>(&mut self) -> IsotopeMap<A, C> {
        match self.isotope.remove(&DbgTypeId::of::<C>()) {
            Some(map) => *map.downcast().expect("TypeId mismatch"),
            None => Vec::new(),
        }
    }

    /// Number of simple components.
    pub fn simple_len(&self) -> usize { self.simple.len() }
    /// Number of distinct isotope component types.
    pub fn isotope_type_count(&self) -> usize { self.isotope.len() }
}

/// Dependency list.
///
/// Items are tuples of `(DbgTypeIdOf::<C>(), storage::simple::builder::<A, C>)`.
pub type DepList = Vec<(DbgTypeId, fn() -> Box<dyn Any>)>;

/// Describes how to instantiate a component based on other component types.
pub struct Initer<A: Archetype, C: comp::SimpleOrIsotope<A>> {
    /// The component function.
    pub f: &'static dyn InitFn<A, C>,
}

/// A function used for [`comp::InitStrategy::Auto`].
///
/// This trait is blanket-implemented for all functions that take up to 32 simple component
/// parameters and output the component value.
pub trait InitFn<A: Archetype, C: comp::SimpleOrIsotope<A>>: Send + Sync + 'static {
    /// Calls the underlying function, building the arguments.
    fn init(&self, dep_getter: DepGetter<'_, A>) -> C;

    /// Returns the component types required by this function.
    fn deps(&self) -> DepList;
}

pub struct DepGetter<'t, A: Archetype> {
    pub(crate) inner:  &'t dyn DepGetterInner<A>,
    pub(crate) entity: A::RawEntity,
}

impl<'t, A: Archetype> Clone for DepGetter<'t, A> {
    fn clone(&self) -> Self { Self { inner: self.inner, entity: self.entity } }
}

impl<'t, A: Archetype> Copy for DepGetter<'t, A> {}

pub(crate) trait DepGetterInner<A: Archetype> {
    fn get(
        &self,
        ty: DbgTypeId,
    ) -> ArcRwLockWriteGuard<parking_lot::RawRwLock, dyn AnySimpleStorage<A>>;
}

macro_rules! impl_simple_init_fn {
    ($($deps:ident),* $(,)?) => {
        impl<
            A: Archetype, C: comp::SimpleOrIsotope<A>,
            $($deps: comp::Simple<A>,)*
        > InitFn<A, C> for fn(
            $(&$deps,)*
        ) -> C {
            fn init(&self, dep_getter: DepGetter<'_, A>) -> C {
                (self)(
                    $(
                        match dep_getter.inner.get(DbgTypeId::of::<$deps>()).get_any(dep_getter.entity) {
                            Some(comp) => comp.downcast_ref::<$deps>().expect("TypeId mismatch"),
                            None => panic!(
                                "Cannot create an entity of type `{}` without explicitly passing a component of type `{}`, or `{}` to invoke its auto-initializer",
                                any::type_name::<A>(),
                                any::type_name::<C>(),
                                any::type_name::<$deps>(),
                            ),
                        },
                    )*
                )
            }

            fn deps(&self) -> DepList {
                vec![
                    $((DbgTypeId::of::<$deps>(), storage::simple::builder::<A, $deps>),)*
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

#[cfg(test)]
mod tests;
