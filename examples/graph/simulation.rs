//! This module defines the graph archetypes
//! and implements the graph simulation code.

/// The main API of this module.
pub struct Bundle;

impl dynec::world::Bundle for Bundle {
    /// Initializes the plugin, registering systems and initializing globals.
    fn register(&self, builder: &mut dynec::world::Builder) {
        // builder.schedule(simulation_flow);
    }

    /// Populates the world with entities.
    /// In actual games, this function should load the world from a save file instead.
    fn populate(&self, world: &mut dynec::World) {
        // First, we populate the world with entities with archetype `Node`.
        // Note that no components from the `render` crate are specified here.
        let farm = world.create::<Node>(dynec::components![ Node =>
            // The `Node` archetype has a single-component of type `Position`.
            Position([0.0, 0.0]),

            // The `Node` archetype has a multi-component of type `Capacity`.
            // This means each `Node` entity can have multiple `Capacity` components,
            // indexed by a small integer of type `Capacity::Discrim` (`ItemType`).
            // The syntax is `@v`, where `v` is a value whose type implements
            // `Iterator<Item = (C::Discrim, C)>` where `C` is the component type.
            @[(CROPS, Capacity(100))],
            // Similarly, `Volume` is a multi-component.
            @[(CROPS, Volume(50))]
        ]);

        // The `world.create_near` method allows providing an entity
        // which hints dynec to allocate the new entity nearby.
        // This hint is useless during world initialization
        // because there are no gaps where the hint is effective for,
        // so entities are always allocated in the same order they are created.
        let factory = world.create::<Node>(dynec::components![ Node =>
            Position([0.0, 1.0]),
            @[(CROPS, Capacity(100)), (FOOD, Capacity(100))],
            @[(FOOD, Volume(100))],
        ]);
        let market = world.create::<Node>(dynec::components![ Node =>
            Position([1.0, 2.0]),
            @[(FOOD, Capacity(200))],
        ]);

        // Then, we populate the world with entities with archetype `Edge`.
        world.create::<Edge>(dynec::components![ Edge =>
            Endpoints{from: farm, to: factory.clone()},
            Power(1.),
            @[(CROPS, Flow(10))],
        ]);
        world.create::<Edge>(dynec::components![ Edge =>
            Endpoints{from: factory, to: market},
            Power(2.),
            @[(CROPS, Flow(10))],
        ]);
    }
}

dynec::archetype! {
    /// Node is an archetype used to identify entities with a specific component set.
    pub Node
}

/// We can define that the component `Position` can be used in the `Node` archetype
/// by implementing `Node: dynec::archetype::Contains<Position>`.
/// The `#[dynec::archetype]` attribute does this to save the boilerplate.
///
/// The `required` argument specifies that the component must be provided during initialization.
/// Alternatively, you can use `optional` which specifies a component does not necessarily exist,
/// or `auto` which populates the component automatically if it implements the `dynec::component::Auto` trait.
#[dynec::component(of = Node, required)]
pub struct Position(pub [f32; 2]);

/// To define a multi-component, we add a `multi(Discrim)` argument to the attribute:
/// [`ItemType`] is defined at the bottom of this file.
/// Multi-components do not have the `required`/`optional`/`auto` argument,
/// because they are all `[]` by default.
/// To put it another way, they are all `optional` on each discriminant.
#[dynec::component(of = Node, isotope = ItemType, init = Default::default/0)]
#[derive(Default)]
pub struct Capacity(pub u32);

#[dynec::component(of = Node, isotope = ItemType, init = || Volume(0))]
pub struct Volume(pub u32);

dynec::archetype! {
    /// Similarly, we define the archetype and components for `Edge`.
    pub Edge
}

/// The `Endpoints` component stores references to the [`Node`] entities that the edge connects.
/// To support permutation and deletion debugging,
/// we need to add `#[entity]` on all fields that transitively contain a reference.
#[dynec::component(of = Edge, required)]
pub struct Endpoints {
    #[entity]
    from: dynec::Entity<Node>,
    #[entity]
    to:   dynec::Entity<Node>,
}

#[dynec::component(of = Edge, required)]
pub struct Power(pub f64);
#[dynec::component(of = Edge, isotope = ItemType)]
pub struct Flow(pub u32);

/// Identifies a type of item.
/// This is a discriminant type used to identify multiple components of the same type.
/// This is useful in systems where multiple item types operate almost independently.
#[derive(Clone, Copy)]
pub struct ItemType(usize);

impl dynec::component::Discrim for ItemType {
    fn from_usize(usize: usize) -> Self { Self(usize) }
    fn to_usize(self) -> usize { self.0 }
}

// Here are a few constants for the different item types.
// Note that they do not need to be constants and can be runtime-defined,
// provided that they have reasonably small indices.
const CROPS: ItemType = ItemType(0);
const FOOD: ItemType = ItemType(1);

/*
#[dynec::system]
fn simulate_flow(
    #[state] item: &ItemType,
    node_pos: impl dynec::system::Reads<Node, Position>,
    #[discrim(*item)] node_cap: impl dynec::system::Reads<Node, Capacity>,
    #[discrim(*item)] node_vol: impl dynec::system::Writes<Node, Volume>,
    edges: impl dynec::system::ReadsAll<Edge, (Endpoints, Power, Flow)>,
    dt: &crate::time::Delta,
) {
    for (&Endpoints { ref from, ref to }, power, flows) in edges.iter() {
        let from_pos = node_pos[from];
        let to_pos = node_pos[to];
        let dist2 = (from_pos.0[0] - to_pos.0[0]).powi(2) + (from_pos.0[1] - to_pos.0[1]).powi(2);
        let multiplier = (dist2 as f64).sqrt() * power;

        for (item, flow) in flows {
            let mut from_vol = node_vol[from][item].unwrap_or_default();
            let mut to_vol = node_vol[to][item].unwrap_or_default();

            let rate = cmp::min(
                ((flow as f64) * multiplier) as u32,
                cmp::min(from_vol, node_cap[to][item].unwrap_or_default() - to_vol),
            );

            from_vol -= rate;
            to_vol += rate;

            node_vol[from][item] = (from_vol > 0).then(|| Some(Volume(from_vol)));
            node_vol[to][item] = (to_vol > 0).then(|| Some(Volume(to_vol)));
        }
    }
}
*/
