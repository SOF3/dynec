//! Tests simple storage access.

use crate::test_util::*;
use crate::{system, system_test, tracer};

#[test]
fn test_simple_fetch() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut comp5: system::WriteSimple<TestArch, Simple5RequiredNoInit>,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        let ent = initials.strong.as_ref().expect("initials.strong is None");

        let comp = comp5.get_mut(ent);
        assert_eq!(comp.0, 7);
        comp.0 += 13;
    }

    let mut world = system_test!(test_system.build(););

    let ent = world.create(crate::comps![@(crate) TestArch => Simple5RequiredNoInit(7)]);
    world.get_global::<InitialEntities>().strong = Some(ent.clone());

    world.execute(&tracer::Log(log::Level::Trace));

    let storage = world.components.get_simple_storage::<TestArch, Simple5RequiredNoInit>();
    let comp = storage.try_get(ent);
    assert_eq!(comp, Some(&Simple5RequiredNoInit(20)));
}
