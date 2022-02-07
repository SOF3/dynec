//! A basic flow simulation.
//!
//! The intended flowchart can be found in `flow.dot`.

use dynec::system;
use dynec::system::Context;
use std::cmp;

fn main() {
    let mut world = dynec::World::default();

    world.schedule(simulate);

    let farm = world.create::<Node>(
        None,
        dynec::component_initials![
            @CROPS => Capacity(100),
            @CROPS => Volume(50),
        ],
    );
    let factory = world.create::<Node>(
        None,
        dynec::component_initials![
            @CROPS => Capacity(100),
            @FOOD => Capacity(100),
            @FOOD => Volume(100),
        ],
    );
    let market = world.create::<Node>(
        None,
        dynec::component_initials![
        @FOOD => Capacity(200),
        ],
    );

    assert_eq!(
        world.get_multi::<Node, Capacity, _, _>(CROPS, farm, |x| *x),
        Some(Capacity(100))
    );
}

#[derive(dynec::Archetype)]
enum Node {}

/// Identifies an item type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ItemType(usize);

const CROPS: ItemType = ItemType(0);
const FOOD: ItemType = ItemType(1);

impl From<usize> for ItemType {
    fn from(id: usize) -> Self {
        ItemType(id)
    }
}

impl From<ItemType> for usize {
    fn from(id: ItemType) -> Self {
        id.0
    }
}

/// A component storing the position of an object.
#[derive(Debug, Clone, Copy, PartialEq, dynec::Component)]
#[component(of = Node, required)]
struct Position([f32; 3]);

/// A component storing the item capacity of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, dynec::Component)]
#[component(of = Node, multi = ItemType, required)]
struct Capacity(u32);

/// A component storing the item volume in a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, dynec::Component)]
#[component(of = Node, multi = ItemType, default)]
struct Volume(u32);

#[derive(dynec::Archetype)]
enum Edge {}

#[derive(dynec::Component)]
#[component(of = Edge, required)]
struct Endpoints(dynec::Entity<Node>, dynec::Entity<Node>);

#[derive(Debug, PartialEq, dynec::Component)]
#[component(of = Edge, multi = ItemType, required)]
struct Flow(u32);

#[derive(Debug, PartialEq, dynec::Component)]
#[component(of = Edge, required)]
struct Power(u32);

#[derive(Debug, Clone)]
struct DeltaTime(u32);

impl dynec::Global for DeltaTime {}

#[system]
fn simulate(
    ctx: impl system::Context
        + system::Reads<Node, Capacity>
        + system::Writes<Node, Volume>
        + system::Reads<Edge, Flow>
        + system::Reads<Edge, Endpoints>
        + system::Super<get_bound>
        + system::Super<compute_delta>,
) {
    for edge in dynec::each!(context, Edge; Flow, Power, Endpoints) {
        let &Endpoints(ref src, ref dest) = edge.get::<Endpoints>();

        for (item, flow) in edge.get_multi::<Flow>() {
            let bound = get_bound(ctx.project(), item, src, dest);

            let delta = compute_delta(ctx.project(), edge.entity(), flow);

            if delta > 0 {
                {
                    ctx.get_multi_mut::<Node, Volume>(item, &src)
                        .expect("delta > 0")
                        .0 -= delta;
                }
                {
                    let volume = ctx.get_multi_mut::<Node, Volume>(item, &dest);
                    match volume {
                        Some(vol) => vol.0 += delta,
                        None => *volume = Some(Volume(delta)),
                    }
                }
            }
        }
    }
}

#[system::subroutine]
fn get_bound(
    ctx: impl system::Context + system::Reads<Node, Capacity> + system::Reads<Node, Volume>,
    item: ItemType,
    src: &dynec::Entity<Node>,
    dest: &dynec::Entity<Node>,
) -> u32 {
    let mut bound = ctx
        .get_multi::<Node, Volume>(item, &src)
        .copied()
        .unwrap_or_default()
        .0;
    if let Some(capacity) = ctx.get_multi::<Node, Capacity>(item, dest) {
        bound = cmp::min(
            bound,
            capacity.0
                - ctx
                    .get_multi::<Node, Volume>(item, dest)
                    .copied()
                    .unwrap_or_default()
                    .0,
        );
    } else {
        bound = 0;
    }
    bound
}

#[system::subroutine]
fn compute_delta(
    ctx: impl system::Context + system::ReadsGlobal<DeltaTime> + system::Reads<Edge, Power>,
    entity: &dynec::Entity<Edge>,
    flow: &Flow,
) -> u32 {
    let dt = ctx.get_global::<DeltaTime>();
    let power = ctx.get::<Edge, Power>(entity);

    flow.0 * power.0 * dt.0
}
