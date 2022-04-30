//! Utilties for dynamic dispatch related to components.

use std::any::{self, Any, TypeId};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::{cmp, hash};

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
    pub(crate) fn simple<A: Archetype, C: component::Simple<A>>() -> Self {
        Identifier { id: TypeId::of::<C>(), name: any::type_name::<C>(), discrim: None }
    }

    pub(crate) fn isotope<A: Archetype, C: component::Isotope<A>>(discrim: C::Discrim) -> Self {
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

    /// Gets a simple component from the map.
    pub(crate) fn remove_simple<C: component::Simple<A>>(&mut self) -> Option<C> {
        let comp = self.map.remove(&Identifier::simple::<A, C>())?;
        let comp = comp.downcast::<C>().expect("TypeId mismatch");
        Some(*comp)
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

/// A function used for [`component::SimpleInitStrategy::Auto`].
///
/// This trait is blanket-implemented for all functions that take up to 32 simple component
/// parameters.
pub trait AutoInitFn<A: Archetype>: 'static {
    /// Calls the underlying function, extracting the arguments.
    fn populate(&self, map: &mut Map<A>);

    /// Returns the component types required by this function.
    fn deps(&self) -> Vec<(TypeId, component::SimpleInitStrategy<A>)>;
}

pub struct ComponentDescriptor {}

macro_rules! impl_auto_init_fn {
    ($($deps:ident),* $(,)?) => {
        impl<
            A: Archetype, C: component::Simple<A>,
            $($deps: component::Simple<A>,)*
        > AutoInitFn<A> for fn(
            $(&$deps,)*
        ) -> C {
            fn populate(&self, map: &mut Map<A>) {
                let populate = (self)(
                    $(map.get_simple::<$deps>().expect("Incorrect dependency sorting"),)*
                );
                map.insert_simple(populate);
            }

            fn deps(&self) -> Vec<(TypeId, component::SimpleInitStrategy<A>)> {
                vec![
                    $((TypeId::of::<$deps>(), <$deps as component::Simple<A>>::INIT_STRATEGY),)*
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
    P1, P2, P3,
    P4, /* P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21,
         * P22, P23, P24, P25, P26, P27, P28, P29, P30, P31, P32 */
);

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_util::*;

    struct Comp1(i32);
    impl_test_simple_component!(
        Comp1,
        presence(Optional),
        init(None),
        finalizer(false),
        entity_refs()
    );

    #[derive(Debug, PartialEq)]
    struct Comp2(i32);
    impl_test_simple_component!(
        Comp2,
        presence(Optional),
        init(None),
        finalizer(false),
        entity_refs()
    );

    #[test]
    fn test_auto_init_fn() {
        let auto_fn = (|comp1: &Comp1| Comp2(comp1.0 + 5)) as fn(&_) -> _;
        let mut map = Map::default();
        map.insert_simple(Comp1(2));
        map.insert_simple(AutoInitFn::<TestArch, Comp2>::call(&auto_fn, &map));
        assert_eq!(map.get_simple::<Comp2>(), Some(&Comp2(7)));
    }
}
