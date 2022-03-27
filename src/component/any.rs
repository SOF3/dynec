//! Utilties for dynamic dispatch related to components.

use std::any::{self, Any, TypeId};
use std::cmp;
use std::collections::BTreeMap;
use std::marker::PhantomData;

use super::Discrim;
use crate::{component, Archetype};

/// Identifies a generic simple or discriminated isotope component type.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Identifier {
    pub(crate) id:      TypeId,
    #[allow(dead_code)] // used for debugging only
    pub(crate) name:    &'static str,
    pub(crate) discrim: Option<usize>,
}

impl Identifier {
    fn simple<A: Archetype, C: component::Simple<A>>() -> Self {
        Identifier { id: TypeId::of::<C>(), name: any::type_name::<C>(), discrim: None }
    }

    fn isotope<A: Archetype, C: component::Isotope<A>>(discrim: C::Discrim) -> Self {
        Identifier {
            id:      TypeId::of::<C>(),
            name:    any::type_name::<C>(),
            discrim: Some(discrim.to_usize()),
        }
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

/// A generic TypeMap of owned simple and isotope components.
///
/// This type is only used in parameter passing, not in the actual storage.
pub struct Map<A: Archetype> {
    map: BTreeMap<Identifier, Box<dyn Any>>,

    _ph: PhantomData<A>,
}

impl<A: Archetype> Default for Map<A> {
    fn default() -> Self { Map { map: BTreeMap::default(), _ph: PhantomData } }
}

impl<A: Archetype> Map<A> {
    /// Inserts a simple component into the map.
    pub fn insert_simple<C: component::Simple<A>>(&mut self, component: C) {
        self.map.insert(Identifier::simple::<A, C>(), Box::new(component));
    }

    /// Inserts an isotope component into the map.
    pub fn insert_isotope<C: component::Isotope<A>>(&mut self, discrim: C::Discrim, component: C) {
        self.map.insert(Identifier::isotope::<A, C>(discrim), Box::new(component));
    }

    /// Gets a simple component from the map.
    pub(crate) fn get_simple<C: component::Simple<A>>(&self) -> Option<&C> {
        self.map.get(&Identifier::simple::<A, C>()).and_then(|c| c.downcast_ref())
    }
}

/// Describes how to instantiate a component based on other component types.
pub struct AutoIniter<A: Archetype, C: component::Simple<A>> {
    pub(crate) f: &'static dyn AutoInitFn<A, C>,
}

impl<A: Archetype, C: component::Simple<A>> AutoIniter<A, C> {
    /// Creates a new [`AutoIniter`] from a function pointer.
    pub fn new(f: &'static dyn AutoInitFn<A, C>) -> Self { AutoIniter { f } }
}

/// A function used for [`component::SimpleInitStrategy::Auto`].
///
/// This trait is blanket-implemented for all functions that take up to 32 simple component
/// parameters.
pub trait AutoInitFn<A: Archetype, C: component::Simple<A>>: 'static {
    /// Calls the underlying function, extracting the arguments.
    fn call(&self, map: &Map<A>) -> C;

    /// Returns the component types required by this function.
    fn for_each_dep(&self, f: &mut dyn FnMut(TypeId));
}

macro_rules! impl_auto_init_fn {
    ($($deps:ident),* $(,)?) => {
        impl<
            A: Archetype, C: component::Simple<A>,
            $($deps: component::Simple<A>,)*
        > AutoInitFn<A, C> for fn(
            $(&$deps,)*
        ) -> C {
            fn call(&self, map: &Map<A>) -> C {
                (self)(
                    $(map.get_simple::<$deps>().expect("Incorrect dependency sorting"),)*
                )
            }

            fn for_each_dep(&self, f: &mut dyn FnMut(TypeId)) {
                for item in [
                    $(TypeId::of::<$deps>(),)*
                ] {
                    f(item);
                }
            }
        }
    }
}

impl_auto_init_fn!();
impl_auto_init_fn!(P1);
impl_auto_init_fn!(P1, P2);
impl_auto_init_fn!(P1, P2, P3);
impl_auto_init_fn!(P1, P2, P3, P4);
impl_auto_init_fn!(P1, P2, P3, P4, P5);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6, P7);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6, P7, P8);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6, P7, P8, P9);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17);
impl_auto_init_fn!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23, P24
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23, P24, P25
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23, P24, P25, P26
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23, P24, P25, P26, P27
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23, P24, P25, P26, P27, P28
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23, P24, P25, P26, P27, P28, P29
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23, P24, P25, P26, P27, P28, P29, P30
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23, P24, P25, P26, P27, P28, P29, P30, P31
);
impl_auto_init_fn!(
    P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
    P22, P23, P24, P25, P26, P27, P28, P29, P30, P31, P32
);
