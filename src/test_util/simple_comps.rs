use std::num::NonZeroU32;

use super::TestArch;
use crate::entity::{self};
use crate::{comp, storage, Entity};

// Test component summary:
// Simple1: optional, depends []
// Simple2: optional, depends on Simple2
// Simple3: optional, depends on Simple1 and Simple2
// Simple4: optional, depends on Simple1 and Simple2
// Simple5: required, no init
// Simple6: required, depends []

/// optional, non-init, depless
#[comp(dynec_as(crate), of = TestArch)]
#[derive(Debug, PartialEq)]
pub struct Simple1OptionalNoDepNoInit(pub i32);

/// optional, depends on Simple1
#[comp(dynec_as(crate), of = TestArch, init = init_comp2/1)]
#[derive(Debug)]
pub struct Simple2OptionalDepends1(pub i32);
fn init_comp2(c1: &Simple1OptionalNoDepNoInit) -> Simple2OptionalDepends1 {
    Simple2OptionalDepends1(c1.0 + 2)
}

/// optional, depends on Simple1 + Simple2
#[comp(
    dynec_as(crate),
    of = TestArch,
    init = |c1: &Simple1OptionalNoDepNoInit, c2: &Simple2OptionalDepends1| Simple3OptionalDepends12(c1.0 * 3, c2.0 * 5),
)]
#[derive(Debug)]
pub struct Simple3OptionalDepends12(pub i32, pub i32);

/// optional, depends on Simple1 + Simple2
#[comp(
    dynec_as(crate),
    of = TestArch,
    init = |c1: &Simple1OptionalNoDepNoInit, c2: &Simple2OptionalDepends1| Simple4Depends12(c1.0 * 7, c2.0 * 8),
)]
#[derive(Debug, PartialEq)]
pub struct Simple4Depends12(pub i32, pub i32);

/// required, non-init
#[comp(dynec_as(crate), of = TestArch, required)]
#[derive(Debug, PartialEq)]
pub struct Simple5RequiredNoInit(pub i32);

/// required, auto-init, depless
#[comp(dynec_as(crate), of = TestArch, required, init = || Simple6RequiredWithInitNoDeps(9))]
#[derive(Debug)]
pub struct Simple6RequiredWithInitNoDeps(pub i32);

/// non-init, has finalizers
#[comp(dynec_as(crate), of = TestArch, finalizer)]
pub struct Simple7WithFinalizerNoinit;

/// a generic component
pub struct SimpleN<const N: usize>(pub i32);

impl<const N: usize> entity::Referrer for SimpleN<N> {
    fn visit_type(arg: &mut entity::referrer::VisitTypeArg) { arg.mark::<Self>(); }
    fn visit_mut<V: entity::referrer::VisitMutArg>(&mut self, _: &mut V) {}
}

impl<const N: usize> comp::SimpleOrIsotope<TestArch> for SimpleN<N> {
    const PRESENCE: comp::Presence = comp::Presence::Optional;
    const INIT_STRATEGY: comp::InitStrategy<TestArch, Self> = comp::InitStrategy::None;

    type Storage = storage::Vec<NonZeroU32, Self>;
}
impl<const N: usize> comp::Simple<TestArch> for SimpleN<N> {
    const IS_FINALIZER: bool = false;
}

/// A simple component with a strong reference to [`TestArch`].
#[comp(dynec_as(crate), of = TestArch)]
pub struct StrongRefSimple(#[entity] pub Entity<TestArch>);
