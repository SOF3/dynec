use std::any::TypeId;

use crate::util::DbgTypeId;
use crate::Archetype;

/// A type that may own entity references (no matter strong or weak).
pub trait Referrer {
    /// Performs the mapping and increments the counter for each entity.
    ///
    /// Each entity reference must be visited only exactly once for each visitor.
    /// As a result, `Referrer` is not implemented for [`std::sync::Arc`]
    /// because it may result in visiting the same entity reference multiple times,
    /// which will lead to incorrect behaviour.
    fn visit(&mut self, arg: &mut ReferrerArg);
}

/// The opaque argument passed through [`Referrer::visit`].
///
/// This type is used to hide the implementation detail from users
/// such that the actual arguments are only visible to the internals.
pub struct ReferrerArg<'t> {
    archetype: DbgTypeId,
    mapping:   &'t [usize],
    counter:   &'t mut usize,
}

impl<A: Archetype> Referrer for super::Weak<A> {
    fn visit(
        &mut self,
        &mut ReferrerArg { archetype, mapping, ref mut counter }: &mut ReferrerArg,
    ) {
        if archetype == TypeId::of::<A>() {
            let &new = mapping
                .get(super::Raw::to_primitive(self.id))
                .expect("Weak reference not in entity. ");
            self.id = <A::RawEntity as super::Raw>::from_primitive(new);
            **counter += 1;
        }
    }
}

impl<A: Archetype> Referrer for super::Entity<A> {
    fn visit(
        &mut self,
        &mut ReferrerArg { archetype, mapping, ref mut counter }: &mut ReferrerArg,
    ) {
        if archetype == TypeId::of::<A>() {
            let &new = mapping
                .get(super::Raw::to_primitive(self.id))
                .expect("Weak reference not in entity. ");
            self.id = <A::RawEntity as super::Raw>::from_primitive(new);
            **counter += 1;
        }
    }
}
