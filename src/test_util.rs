#![allow(missing_docs)]
#![allow(clippy::too_many_arguments)]

use std::collections::BTreeSet;
use std::num::NonZeroU32;

use parking_lot::Once;

use crate::entity::ealloc;
use crate::{system, Archetype};

mod event_tracer;
pub use event_tracer::EventTracer;

mod clock;
pub use clock::{Clock, Tick};

mod anti_semaphore;
pub use anti_semaphore::AntiSemaphore;

pub(crate) fn init() {
    static SET_LOGGER_ONCE: Once = Once::new();
    SET_LOGGER_ONCE.call_once(env_logger::init);
}

/// The default test archetype.
pub enum TestArch {}

impl Archetype for TestArch {
    type RawEntity = NonZeroU32;
    type Ealloc =
        ealloc::Recycling<NonZeroU32, BTreeSet<NonZeroU32>, ealloc::ThreadRngShardAssigner>;
}

mod simple_comps;
pub use simple_comps::*;

mod isotope_comps;
pub use isotope_comps::*;

mod globals;
pub use globals::*;

/// A dummy system used for registering all non-entity-referencing test components.
#[system(dynec_as(crate))]
pub fn use_all_bare(
    _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    _comp2: system::ReadSimple<TestArch, Simple2OptionalDepends1>,
    _comp3: system::ReadSimple<TestArch, Simple3OptionalDepends12>,
    _comp4: system::ReadSimple<TestArch, Simple4Depends12>,
    _comp5: system::ReadSimple<TestArch, Simple5RequiredNoInit>,
    _comp6: system::ReadSimple<TestArch, Simple6RequiredWithInitNoDeps>,
    _comp_final: system::ReadSimple<TestArch, Simple7WithFinalizerNoinit>,
    _iso1: system::ReadIsotopeFull<TestArch, IsoNoInit>,
    _iso2: system::ReadIsotopeFull<TestArch, IsoWithInit>,
    #[dynec(global)] _agg: &Aggregator,
) {
}

/// A dummy system with minimally simple dependencies.
#[system(dynec_as(crate))]
pub fn use_comp_n(
    _comp0: system::ReadSimple<TestArch, SimpleN<0>>,
    _comp1: system::ReadSimple<TestArch, SimpleN<1>>,
    _comp2: system::ReadSimple<TestArch, SimpleN<2>>,
    _comp3: system::ReadSimple<TestArch, SimpleN<3>>,
    _comp4: system::ReadSimple<TestArch, SimpleN<4>>,
    _comp5: system::ReadSimple<TestArch, SimpleN<5>>,
    _comp6: system::ReadSimple<TestArch, SimpleN<6>>,
    _comp7: system::ReadSimple<TestArch, SimpleN<7>>,
    _comp8: system::ReadSimple<TestArch, SimpleN<8>>,
    _comp9: system::ReadSimple<TestArch, SimpleN<9>>,
    _comp10: system::ReadSimple<TestArch, SimpleN<10>>,
    _comp11: system::ReadSimple<TestArch, SimpleN<11>>,
    _comp12: system::ReadSimple<TestArch, SimpleN<12>>,
    _comp13: system::ReadSimple<TestArch, SimpleN<13>>,
    _comp14: system::ReadSimple<TestArch, SimpleN<14>>,
    _comp15: system::ReadSimple<TestArch, SimpleN<15>>,
) {
}
