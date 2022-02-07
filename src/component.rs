//! A component is some data stored attached to an entity.

use std::any::{self, Any, TypeId};
use std::collections::BTreeMap;
use std::marker::PhantomData;

use crate::{archetype, entity, Archetype};

/// A component is some data stored attached to an entity.
pub trait Component: Sized + 'static {
    /// Casts the component as a `dyn Any` trait object.
    fn as_any(&self) -> &dyn Any;

    /// Casts the component as a `dyn Any` trait object.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// The default initializer for a component.
    fn factory() -> Factory<Self>;
}

/// A component with only one instance for each object.
pub trait Single: Component {}

/// Marker trait for single components that are not optional.
///
/// `<T as SingleMust>::factory()` must not return `Factory::Optional`.
pub trait SingleMust: Single {}

/// A component with a dynamic number of instances for each object.
pub trait Multi: Component {
    /// The type used to distinguish different instances of this component.
    ///
    /// For example, if an entity has one component instance for each `XxxId`,
    /// the Ord should be `XxxId`.
    type Ord: From<usize> + Into<usize>;
}

/// Initial components for an entity.
pub struct Initials<A: Archetype> {
    pub(crate) single: BTreeMap<TypeId, SingleEntry>,
    pub(crate) multi: BTreeMap<TypeId, MultiEntry>,
    _ph: PhantomData<&'static A>,
}

impl<A: Archetype> Default for Initials<A> {
    fn default() -> Self {
        Self {
            single: BTreeMap::new(),
            multi: BTreeMap::new(),
            _ph: PhantomData,
        }
    }
}

pub(crate) struct SingleEntry {
    pub(crate) type_name: &'static str,
    pub(crate) value: Box<dyn Any>,
}

pub(crate) struct MultiEntry {
    pub(crate) type_name: &'static str,
    pub(crate) values: BTreeMap<usize, Box<dyn Any>>,
}

impl<A: Archetype> Initials<A> {
    /// Adds a single-component to the builder.
    #[must_use = "Initials::single does not modify the receiver value"]
    pub fn single<C: Single>(mut self, value: C) -> Self
    where
        A: archetype::Contains<C>,
    {
        self.single.insert(
            TypeId::of::<C>(),
            SingleEntry {
                type_name: any::type_name::<C>(),
                value: Box::new(value),
            },
        );
        self
    }

    /// Adds a multi-component to the builder.
    #[must_use = "Initials::multi does not modify the receiver value"]
    pub fn multi<C: Multi>(mut self, ord: C::Ord, value: C) -> Self
    where
        A: archetype::Contains<C>,
    {
        let ord: usize = ord.into();

        let entry = self
            .multi
            .entry(TypeId::of::<C>())
            .or_insert_with(|| MultiEntry {
                type_name: any::type_name::<C>(),
                values: BTreeMap::new(),
            });
        entry.values.insert(ord, Box::new(value));
        self
    }
}

/// Constructs a literal [`Initials`].
#[macro_export]
macro_rules! component_initials {
    ($( $( @ $ord :expr => )? $comp:expr ),* $(,)?) => {{
        let mut initials = $crate::component::Initials::default();
        $(
            $crate::component_initials!(@COMPONENT initials; $(@MULTI $ord =>)? $comp );
        )*
        initials
    }};

    (@COMPONENT $initials:ident; @MULTI $mord:expr => $mcomp:expr) => {
        $initials = $initials.multi($mord, $mcomp);
    };
    (@COMPONENT $initials:ident; $scomp:expr) => {
        $initials = $initials.single($scomp);
    };
}

/// Default initializer for a component.
pub enum Factory<C: Component> {
    /// If the component is not specified in [`Initials`], the component is not created.
    Optional,
    /// Panic if [`Initials`] does not contain the component.
    RequiredInput,
    /// Call the function to initialize the component if not in [`Initials`].
    AutoInit(Box<dyn Fn() -> C>),
}
