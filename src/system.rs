//! Resource management for system scheduling.

use std::any::Any;

use crate::{archetype, component, entity, Archetype, Component, Entity, Global, World};

#[doc(inline)]
pub use dynec_codegen::subroutine;

/// A system is a routine operating on a subset of components
/// that can be scheduled for execution.
pub trait System: Sized + 'static {
    /// The context from which the system function can access the world.
    type Context: Context;

    /// Executes the system.
    fn run(ctx: &World);

    /// Returns meta info about the system.
    fn meta() -> Meta;
}

/// A subroutine is a function that can be called from a system,
/// with its own requirements of world resources.
///
/// Subroutines allow components and global resources to be requested
/// without repeating all types in the parent system.
pub trait Subroutine: Sized + 'static {
    /// The projection context required by the subroutine.
    /// Can be created by calling [`Context::project`].
    type Context: Context;

    /// Returns meta info about the subroutine.
    fn meta() -> Meta;
}

pub(crate) trait AnySystem {
    fn run(&self, ctx: &World);
    fn meta(&self) -> Meta;
}

impl<T: System> AnySystem for T {
    fn run(&self, ctx: &World) {
        <T as System>::run(ctx)
    }

    fn meta(&self) -> Meta {
        T::meta()
    }
}

/// Metadata for a system.
pub struct Meta {}

/// A context object passed as the parameter to a system function.
///
/// The context can be used to query the world for components declared in [`Reads`] and [`Writes`].
/// It can also be passed to other systems declared in [`Super`].
#[allow(clippy::mut_from_ref)]
pub trait Context {
    /// Returns a single-component.
    ///
    /// # Panics
    /// Panics if the component storage is writable
    /// and the result from [`Context::get_mut`]
    /// on the same `A` and `C` is not dropped yet.
    fn get<A: Archetype, C: Component>(&self, entity: &Entity<A>) -> &C
    where
        A: archetype::Contains<C>,
        C: component::Single,
        Self: Reads<A, C>;

    /// Returns a single-component.
    ///
    /// # Returns
    /// Returns `None` if `C::factory()` returns `Optional`
    /// and the component has not been initialized for this entity yet
    /// or has been set to `None`.
    ///
    /// # Panics
    /// Panics if the component storage is writable
    /// and the result from [`Context::get_mut`]
    /// on the same `A` and `C` is not dropped yet.
    fn try_get<A: Archetype, C: Component>(&self, entity: &Entity<A>) -> Option<&C>
    where
        A: archetype::Contains<C>,
        C: component::Single,
        Self: Reads<A, C>;

    /// Returns a multi-component.
    ///
    /// # Returns
    /// Returns `None` if the component of this `ord`
    /// has not been initialized for this entity yet or has been set to `None`.
    ///
    /// # Panics
    /// Panics if the component storage is writable
    /// and the result from [`Context::get_mut`]
    /// on the same `A`, `C` and `ord` is not dropped yet.
    fn get_multi<A: Archetype, C: Component>(
        &self,
        ord: <C as component::Multi>::Ord,
        entity: &Entity<A>,
    ) -> Option<&C>
    where
        A: archetype::Contains<C>,
        C: component::Multi,
        Self: Reads<A, C>;

    /// Returns a writable reference to a single-component.
    ///
    /// # Returns
    /// Returns a smart pointer that reflects the component state,
    /// and updates the component when it has been dropped.
    /// The smart pointer dereferences to `&mut Option<C>`.
    ///
    /// Note that the drop behaviour of the smart pointer is transparent to users
    /// because other code cannot access the component column before the smart pointer is dropped.
    ///
    /// # Panics
    /// Panics if the result from another call to [`Context::get_mut`]
    /// on the same `A` and `C` is not dropped yet.
    fn get_mut<A: Archetype, C: Component>(&self, entity: &Entity<A>) -> &mut Option<C>
    // TODO change to impl DerefMut<Target = Option<C>>
    where
        A: archetype::Contains<C>,
        C: component::Single,
        Self: Writes<A, C>;

    /// Returns a writable reference to a multi-component.
    ///
    /// # Returns
    /// Returns a smart pointer that reflects the component state,
    /// and updates the component when it has been dropped.
    /// The smart pointer dereferences to `&mut Option<C>`.
    ///
    /// # Panics
    /// Panics if the result from another call to [`Context::get_mut`]
    /// on the same `ord` is not dropped yet.
    fn get_multi_mut<A: Archetype, C: Component>(
        &self,
        ord: <C as component::Multi>::Ord,
        entity: &Entity<A>,
    ) -> &mut Option<C>
    // TODO change to impl DerefMut<Target = Option<C>>
    where
        A: archetype::Contains<C>,
        C: component::Multi,
        Self: Writes<A, C>;

    /// Returns a global resource.
    ///
    /// # Panics
    /// Panics if the result from [`Context::get_global_mut`] of the same resource is not dropped yet.
    fn get_global<G: Global>(&self) -> &G
    where
        Self: ReadsGlobal<G>;

    /// Returns a writable reference to a global resource.
    ///
    /// # Panics
    /// Panics if the result from another call to [`Context::get_global_mut`] is not dropped yet.
    fn get_global_mut<G: Global>(&self) -> &mut G
    where
        Self: WritesGlobal<G>;

    /// Projects the context into a subroutine context.
    ///
    /// This operation is a no-op, only rewrapping at the type level.
    fn project<S: Subroutine>(&self) -> S::Context
    where
        Self: Super<S>;
}

/// Allows using [`Context::get`] or [`Context::get_multi`] on the specified component.
pub trait Reads<A: Archetype, C: Component>: Context
where
    A: archetype::Contains<C>,
{
}

/// Allows using [`Context::get_mut`] or [`Context::get_multi_mut`] on the specified component.
pub trait Writes<A: Archetype, C: Component>: Context
where
    A: archetype::Contains<C>,
{
}

/// Allows using [`Context::get_global`] or [`Context::get_multi_mut`] on the specified component.
pub trait ReadsGlobal<G: Global>: Context {}

/// Allows using [`Context::get_global`] or [`Context::get_multi_mut`] on the specified component.
pub trait WritesGlobal<G: Global>: Context {}

/// Allows using [`Context::project`] to call subroutines.
pub trait Super<S: Subroutine>: Context {}

/// Wraps a context on a specific entity.
pub trait SpecificContextt<A: Archetype> {
    /// Returns the entity that this wrapper acts on.
    fn entity(&self) -> &dyn entity::Ref<A>;

    /// Returns a single-component.
    ///
    /// # Panics
    /// Panics if the component storage is writable
    /// and the result from [`Context::get_mut`]
    /// on the same `A` and `C` is not dropped yet.
    fn get<C: Component>(&self) -> &C
    where
        A: archetype::Contains<C>,
        C: component::Single,
        Self: Reads<A, C>;

    /// Returns a single-component.
    ///
    /// # Returns
    /// Returns `None` if `C::factory()` returns `Optional`
    /// and the component has not been initialized for this entity yet
    /// or has been set to `None`.
    ///
    /// # Panics
    /// Panics if the component storage is writable
    /// and the result from [`Context::get_mut`]
    /// on the same `A` and `C` is not dropped yet.
    fn try_get<C: Component>(&self) -> Option<&C>
    where
        A: archetype::Contains<C>,
        C: component::Single,
        Self: Reads<A, C>;

    /// Returns a multi-component.
    ///
    /// # Returns
    /// Returns `None` if the component of this `ord`
    /// has not been initialized for this entity yet or has been set to `None`.
    ///
    /// # Panics
    /// Panics if the component storage is writable
    /// and the result from [`Context::get_mut`]
    /// on the same `A`, `C` and `ord` is not dropped yet.
    fn get_multi<C: Component>(&self, ord: <C as component::Multi>::Ord) -> Option<&C>
    where
        A: archetype::Contains<C>,
        C: component::Multi,
        Self: Reads<A, C>;

    /// Returns a writable reference to a single-component.
    ///
    /// # Returns
    /// Returns a smart pointer that reflects the component state,
    /// and updates the component when it has been dropped.
    /// The smart pointer dereferences to `&mut Option<C>`.
    ///
    /// Note that the drop behaviour of the smart pointer is transparent to users
    /// because other code cannot access the component column before the smart pointer is dropped.
    ///
    /// # Panics
    /// Panics if the result from another call to [`Context::get_mut`]
    /// on the same `A` and `C` is not dropped yet.
    fn get_mut<C: Component>(&self) -> &mut Option<C>
    // TODO change to impl DerefMut<Target = Option<C>>
    where
        A: archetype::Contains<C>,
        C: component::Single,
        Self: Writes<A, C>;

    /// Returns a writable reference to a multi-component.
    ///
    /// # Returns
    /// Returns a smart pointer that reflects the component state,
    /// and updates the component when it has been dropped.
    /// The smart pointer dereferences to `&mut Option<C>`.
    ///
    /// # Panics
    /// Panics if the result from another call to [`Context::get_mut`]
    /// on the same `ord` is not dropped yet.
    fn get_multi_mut<C: Component>(&self, ord: <C as component::Multi>::Ord) -> &mut Option<C>
    // TODO change to impl DerefMut<Target = Option<C>>
    where
        A: archetype::Contains<C>,
        C: component::Multi,
        Self: Writes<A, C>;
}
