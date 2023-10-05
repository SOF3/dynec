#![allow(clippy::ptr_arg)]

use crate::system;
use crate::test_util::*;

#[system(dynec_as(crate))]
fn common_test_system(
    _comp3: system::ReadSimple<TestArch, Simple3OptionalDepends12>,
    _comp4: system::WriteSimple<TestArch, Simple4Depends12>,
    _comp5: system::ReadSimple<TestArch, Simple5RequiredNoInit>,
    _comp6: system::ReadSimple<TestArch, Simple6RequiredWithInitNoDeps>,
    #[dynec(isotope(discrim = [TestDiscrim1(11), TestDiscrim1(17)]))]
    _iso1: system::ReadIsotopePartial<TestArch, IsoNoInit, [TestDiscrim1; 2]>,
    #[dynec(global)] _aggregator: &mut Aggregator,
    #[dynec(global)] _initials: &InitialEntities,
) {
}

mod dependencies;
mod entity_iter;
mod globals;
mod isotope;
mod offline_buffer;
mod simple;
