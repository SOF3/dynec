//! An example "render" plugin that adds a `render` system
//! with custom components to nodes and edges for caching render values.
//!
//! For simplicity, this example just computes the rendering color
//! so that integration testing is possible on headless platforms.
//!
//! Note that this is a leaf module,
//! i.e. other modules (other than main) have no dependency on this module.
//! This means this module can be extracted to a separate crate.
//! The purpose is to demonstrate that
//! dynec can be used in multi-crate games.

use crate::simulation;

/// The main API of this module.
pub struct Bundle;

impl dynec::world::Bundle for Bundle {
    /// Initializes the plugin, registering systems and initializing globals.
    fn register(&self, _builder: &mut dynec::world::Builder) {
        //builder.schedule(render);
    }

    /// Populates the world with entities.
    /// In actual games, this function should load the world from a save file instead.
    fn populate(&self, _: &mut dynec::World) {}
}

/// This module computes the color of a node/edge and stores it in the entity.
///
/// As a component declared in a separate and independent module,
/// it probably does not make sense to use `required` here
/// unless we have a separate mechanism to tell the creation module
/// the extra components we want to create.
/// Instead, we use `auto` here,
/// which allows dynamic generation of the component values from the initial components.
#[dynec::comp(of = simulation::Node, of = simulation::Edge, init = || RenderColor { color: [0.0, 0.0, 0.0] })]
struct RenderColor {
    color: [f32; 3],
}

/*
#[dynec::system]
fn render(
    nodes: impl dynec::system::ReadsAll<simulation::Node, (simulation::Capacity, simulation::Volume)>,
    node_colors: impl dynec::system::Writes<simulation::Node, RenderColor>,
) {
    for (node, (caps, vols)) in nodes.iter_with_entity() {
        let color = RenderColor { color: [0.0, 0.0, 0.0] };

        for (_item, (cap, vol)) in dynec::zip_multi!(caps, vols) {
            for i in 0..3 {
                color.color[i] += (vol.0 as f32) / (cap.0 as f32);
            }
        }

        node_colors[node] = color;
    }
}
*/
