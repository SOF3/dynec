//! Tests simple storage access.

use crate::entity::{generation, Ref as _};
use crate::test_util::*;
use crate::{system, system_test, tracer};

#[test]
fn test_simple_fetch() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut comp5: system::WriteSimple<TestArch, Simple5RequiredNoInit>,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        let ent = initials.strong.as_ref().expect("initials.strong is assigned during init");

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

#[test]
fn test_get_many() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut comp5: system::WriteSimple<TestArch, Simple5RequiredNoInit>,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        let strong = initials.strong.as_ref().expect("initials.stonrg is assigned during init");
        let weak = initials.weak.as_ref().expect("initials.weak is assigned during init");

        let [strong_comp, weak_comp] = comp5.get_many_mut([strong.as_ref(), weak.as_ref()]);
        strong_comp.0 += 11;
        weak_comp.0 += 13;
    }

    let mut world = system_test!(test_system.build(););

    let strong = world.create(crate::comps![@(crate) TestArch => Simple5RequiredNoInit(7)]);
    world.get_global::<InitialEntities>().strong = Some(strong.clone());

    let weak = world.create(crate::comps![@(crate) TestArch => Simple5RequiredNoInit(3)]);
    world.get_global::<InitialEntities>().weak =
        Some(weak.weak(world.get_global::<generation::StoreMap>()));

    world.execute(&tracer::Log(log::Level::Trace));

    let storage = world.components.get_simple_storage::<TestArch, Simple5RequiredNoInit>();
    assert_eq!(storage.try_get(&strong), Some(&Simple5RequiredNoInit(7 + 11)));
    assert_eq!(storage.try_get(&weak), Some(&Simple5RequiredNoInit(3 + 13)));
}
