//! Tests EntityCreator and EntityDeleter.

use crate::entity::{deletion, generation};
use crate::test_util::*;
use crate::{global, system, system_test, tracer, world, Entity};

#[test]
fn test_entity_create() {
    #[global(dynec_as(crate), initial = Step::Create)]
    enum Step {
        Create,
        Access,
    }

    #[derive(Debug, PartialEq, Eq, Hash)]
    struct LatePartition;

    #[system(dynec_as(crate), before(LatePartition))]
    fn entity_creator_system(
        mut entity_creator: system::EntityCreator<TestArch>,
        #[dynec(global(maybe_uninit(TestArch)))] initials: &mut InitialEntities,
        #[dynec(global)] step: &Step,
    ) {
        match step {
            Step::Create => {
                initials.strong = Some(
                    entity_creator
                        .create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(5)]),
                );
            }
            Step::Access => {}
        }
    }

    #[system(dynec_as(crate))]
    fn comp_access_system(
        comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
        #[dynec(global)] initials: &InitialEntities,
        #[dynec(global)] step: &Step,
    ) {
        match step {
            Step::Create => {
                assert!(initials.strong.is_none());
            }
            Step::Access => {
                let ent = initials.strong.as_ref().expect("initials.strong should have been set");
                comp1.try_get(ent).expect("initials.strong should have been initialized");
            }
        }
    }

    #[system(dynec_as(crate), after(LatePartition))]
    fn late_comp_access_system(
        // component storage does not require maybe_uninit unless the component has something like `Option<Box<Self>>`
        comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
        #[dynec(global(maybe_uninit(TestArch)))] initials: &InitialEntities,
        #[dynec(global)] step: &Step,
    ) {
        match step {
            Step::Create => {
                let ent = initials.strong.as_ref().expect("initials.strong should have been set");
                assert!(comp1.try_get(ent).is_none(), "entity should be in pre-initialize state");
            }
            Step::Access => {
                let ent = initials.strong.as_ref().expect("initials.strong should have been set");
                comp1.try_get(ent).expect("initials.strong should have been initialized");
            }
        }
    }

    let mut world = system_test!(comp_access_system.build(), late_comp_access_system.build(), entity_creator_system.build(););

    world.execute(&tracer::Log(log::Level::Trace));
    *world.get_global::<Step>() = Step::Access;
    world.execute(&tracer::Log(log::Level::Trace));

    let ent = {
        let initials = world.get_global::<InitialEntities>();
        let ent = initials.strong.as_ref().expect("initials.strong missing");
        ent.clone()
    };
    let storage = world.components.get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>();
    let comp1 = storage.try_get(&ent);
    assert_eq!(comp1, Some(&Simple1OptionalNoDepNoInit(5)));
}

#[test]
#[should_panic = "Scheduled systems have a cyclic dependency: "]
fn test_entity_create_conflict() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_creator: system::EntityCreator<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        initials.strong = Some(
            entity_creator
                .create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(5)]),
        );
    }

    let mut world = system_test!(test_system.build(););

    world.execute(&tracer::Log(log::Level::Trace));

    let ent = {
        let initials = world.get_global::<InitialEntities>();
        let ent = initials.strong.as_ref().expect("initials.strong missing");
        ent.clone()
    };
    let storage = world.components.get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>();
    let comp1 = storage.try_get(&ent);
    assert_eq!(comp1, Some(&Simple1OptionalNoDepNoInit(5)));
}

#[test]
fn test_entity_delete() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        entity_deleter.queue(initials.strong.take().expect("initials.strong missing"));
    }

    let mut world = system_test!(test_system.build(););
    let ent = world.create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(7)]);
    let weak = ent.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().strong = Some(ent);

    world.execute(&tracer::Log(log::Level::Trace));

    let storage = world.components.get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>();
    let comp1 = storage.try_get(&weak);
    assert_eq!(comp1, None);
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    system dynec::system::offline_buffer::tests::test_system. All strong \
                    references to an entity must be dropped before queuing for deletion and \
                    removing all finalizers."
)]
fn test_entity_delete_send_system_leak() {
    #[system(dynec_as(crate))]
    fn test_system(
        #[dynec(local(initial = None, entity))] entity: &mut Option<Entity<TestArch>>,
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        if let Some(ent) = initials.strong.take() {
            *entity = Some(ent);
        }

        if let Some(ent) = entity {
            entity_deleter.queue(&*ent);
        }
    }

    let mut builder = world::Builder::new(0);
    builder.schedule(test_system.build());

    let mut world = builder.build();

    let ent = world.create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(7)]);
    let weak = ent.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().strong = Some(ent);

    world.execute(&tracer::Log(log::Level::Trace));

    let storage = world.components.get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>();
    let comp1 = storage.try_get(&weak);
    assert_eq!(comp1, None);
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    system dynec::system::offline_buffer::tests::test_system. All strong \
                    references to an entity must be dropped before queuing for deletion and \
                    removing all finalizers."
)]
fn test_entity_delete_unsend_system_leak() {
    #[system(dynec_as(crate), thread_local)]
    fn test_system(
        #[dynec(local(initial = None, entity))] entity: &mut Option<Entity<TestArch>>,
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        if let Some(ent) = initials.strong.take() {
            *entity = Some(ent);
        }

        if let Some(ent) = entity {
            entity_deleter.queue(&*ent);
        }
    }

    let mut builder = world::Builder::new(0);
    builder.schedule_thread_unsafe(test_system.build());

    let mut world = builder.build();

    let ent = world.create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(7)]);
    let weak = ent.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().strong = Some(ent);

    world.execute(&tracer::Log(log::Level::Trace));

    let storage = world.components.get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>();
    let comp1 = storage.try_get(&weak);
    assert_eq!(comp1, None);
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    global state dynec::test_util::globals::InitialEntities. All strong \
                    references to an entity must be dropped before queuing for deletion and \
                    removing all finalizers."
)]
fn test_entity_delete_sync_global_leak() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        entity_deleter.queue(initials.strong.as_ref().expect("initials.strong missing"));
    }

    let mut world = system_test!(test_system.build(););
    let ent = world.create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(7)]);
    world.get_global::<InitialEntities>().strong = Some(ent);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    global state dynec::test_util::globals::InitialEntities. All strong \
                    references to an entity must be dropped before queuing for deletion and \
                    removing all finalizers."
)]
fn test_entity_delete_unsync_global_leak() {
    #[system(dynec_as(crate), thread_local)]
    fn test_system(
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global(thread_local))] initials: &mut InitialEntities,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        entity_deleter.queue(initials.strong.as_ref().expect("initials.strong missing"));
    }

    let mut builder = world::Builder::new(0);
    builder.schedule_thread_unsafe(test_system.build());

    let mut world = builder.build();

    let ent = world.create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(7)]);
    world.get_global_unsync::<InitialEntities>().strong = Some(ent);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    dynec::test_util::TestArch / dynec::test_util::simple_comps::StrongRefSimple. \
                    All strong references to an entity must be dropped before queuing for \
                    deletion and removing all finalizers."
)]
fn test_entity_delete_simple_leak() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _srs: system::ReadSimple<TestArch, StrongRefSimple>,
    ) {
        let entity = initials.weak.as_ref().expect("initials.strong missing");
        entity_deleter.queue(entity);
    }

    let mut builder = world::Builder::new(0);
    builder.schedule(test_system.build());

    let mut world = builder.build();

    let ent = world.create(crate::comps![@(crate) TestArch =>]);
    let weak = ent.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().weak = Some(weak);

    world.create(crate::comps![@(crate) TestArch => StrongRefSimple(ent)]);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    dynec::test_util::TestArch / \
                    dynec::test_util::isotope_comps::StrongRefIsotope # TestDiscrim1(29). All \
                    strong references to an entity must be dropped before queuing for deletion \
                    and removing all finalizers."
)]
fn test_entity_delete_isotope_leak() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _sri: system::ReadIsotopeFull<TestArch, StrongRefIsotope>,
    ) {
        let entity = initials.weak.as_ref().expect("initials.strong missing");
        entity_deleter.queue(entity);
    }

    let mut builder = world::Builder::new(0);
    builder.schedule(test_system.build());

    let mut world = builder.build();

    let ent = world.create(crate::comps![@(crate) TestArch =>]);
    let weak = ent.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().weak = Some(weak);

    world.create(crate::comps![@(crate) TestArch => @(TestDiscrim1(29), StrongRefIsotope(ent))]);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
fn test_entity_finalizer_delete() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        deletion_flags: system::ReadSimple<TestArch, deletion::Flag>,
        mut comp_final: system::WriteSimple<TestArch, Simple7WithFinalizerNoinit>,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        let ent = initials.strong.as_ref().expect("initials.strong missing");
        if deletion_flags.try_get(ent).is_some() {
            comp_final.set(ent, None);
            initials.strong = None;
        } else {
            entity_deleter.queue(ent);
        }
    }

    let mut world = system_test!(test_system.build(););

    for _ in 0..3 {
        let ent = world.create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(13), Simple7WithFinalizerNoinit]);
        let weak = ent.weak(world.get_global::<generation::StoreMap>());
        world.get_global::<InitialEntities>().strong = Some(ent);

        // first iteration
        world.execute(&tracer::Log(log::Level::Trace));

        let storage = world.components.get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>();
        let comp1 = storage.try_get(&weak);
        assert_eq!(comp1, Some(&Simple1OptionalNoDepNoInit(13)));

        // second iteration
        world.execute(&tracer::Log(log::Level::Trace));

        let storage = world.components.get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>();
        let comp1 = storage.try_get(&weak);
        assert_eq!(comp1, None);
    }
}
