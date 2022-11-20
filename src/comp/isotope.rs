use super::{Discrim, IsotopeInitFn, IsotopeIniter};
use crate::{entity, Archetype, Storage};

/// An isotope component may have multiple instances per entity.
///
/// See the [module-level documentation](mod@crate::comp) for more information.
pub trait Isotope<A: Archetype>: entity::Referrer + Send + Sync + Sized + 'static {
    /// The initialization strategy for this component.
    const INIT_STRATEGY: IsotopeInitStrategy<A>;

    /// The discriminant type.
    type Discrim: Discrim;

    /// The storage type used for storing this simple component.
    type Storage: Storage<RawEntity = A::RawEntity, Comp = Self>;
}

/// Describes how an isotope component is auto-initialized.
pub enum IsotopeInitStrategy<A: Archetype> {
    /// The component is not auto-initialized.
    None,
    /// The component should be auto-initialized using the [`IsotopeIniter`]
    /// if it is not given in the creation args.
    Auto(IsotopeIniter<A>),
}

impl<A: Archetype> IsotopeInitStrategy<A> {
    /// Constructs an auto-initializing init strategy from a closure.
    pub fn auto(f: &'static impl IsotopeInitFn<A>) -> Self { Self::Auto(IsotopeIniter { f }) }
}
