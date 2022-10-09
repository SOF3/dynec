use std::hash;
use std::num::NonZeroU32;

use super::{Ref, UnclonableRef};
use crate::TestArch;

// ensure that Ref<Archetype = A> for a fixed `A` must be object-safe.
fn test_object_safety() {
    let _: &dyn Ref<Archetype = TestArch> =
        &UnclonableRef::new(NonZeroU32::new(1).expect("1 != 0"));
}

// Make sure that Entity is not collatable and hashable,
// because order and hash values may change after permutation.
// However, Eq is fine because equality are preserved over permutation.
static_assertions::assert_not_impl_any!(super::Entity<TestArch>: PartialOrd, hash::Hash);
