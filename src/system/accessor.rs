use std::{any, ops};

use crate::{comp, entity, Archetype};

/// Provides access to a simple component in a specific archetype.
pub trait ReadSimple<A: Archetype, C: comp::Simple<A>> {
    /// Returns an immutable reference to the component for the specified entity,
    /// or `None` if the component is not present in the entity.
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C>;

    /// Returns an immutable reference to the component for the specified entity.
    ///
    /// # Panics
    /// This method panics if the entity is not fully initialized yet.
    /// This happens when an entity is newly created and the cycle hasn't joined yet.
    fn get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> &C
    where
        C: comp::Must<A>,
    {
        match self.try_get(entity) {
            Some(comp) => comp,
            None => panic!(
                "Component {}/{} implements comp::Must but is not present",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        }
    }
}

/// Provides access to a simple component in a specific archetype.
pub trait WriteSimple<A: Archetype, C: comp::Simple<A>>: ReadSimple<A, C> {
    /// Returns a mutable reference to the component for the specified entity,
    /// or `None` if the component is not present in the entity.
    ///
    /// Note that this method returns `Option<&mut C>`, not `&mut Option<C>`.
    /// This means setting the Option itself to `Some`/`None` will not modify any stored value.
    /// Use [`WriteSimple::set`] to add/remove a component.
    fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C>;

    /// Returns a mutable reference to the component for the specified entity.
    ///
    /// This method is infallible, assuming [`comp::Must`] is only implemented
    /// for components with [`Required`](comp::SimplePresence::Required) presence.
    fn get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> &mut C
    where
        C: comp::Must<A>,
    {
        match self.try_get_mut(entity) {
            Some(comp) => comp,
            None => panic!(
                "Component {}/{} implements comp::Must but is not present",
                any::type_name::<A>(),
                any::type_name::<C>(),
            ),
        }
    }

    /// Overwrites the component for the specified entity.
    ///
    /// Passing `None` to this method removes the component from the entity.
    /// This leads to a panic for components with [`comp::SimplePresence::Required`] presence.
    fn set<E: entity::Ref<Archetype = A>>(&mut self, entity: E, value: Option<C>) -> Option<C>;
}

/// Provides access to an isotope component in a specific archetype.
///
/// `K` is the type used to index the discriminant.
/// For partial
pub trait ReadIsotope<A: Archetype, C: comp::Isotope<A>, K = C::Discrim> {
    /// Return value of [`try_get`](Self::try_get) and [`get`](Self::get).
    type Get<'t>: ops::Deref<Target = C> + 't
    where
        Self: 't;
    /// Retrieves the component for the given entity and discriminant.
    ///
    /// This method is infallible for correctly implemented `comp::Must`,
    /// which returns the auto-initialized value for missing components.
    fn get<E: entity::Ref<Archetype = A>>(&self, entity: E, discrim: K) -> Self::Get<'_>
    where
        C: comp::Must<A>;

    /// Returns an immutable reference to the component for the specified entity and discriminant,
    /// or the default value for isotopes with a default initializer or `None`
    /// if the component is not present in the entity.
    fn try_get<E: entity::Ref<Archetype = A>>(
        &self,
        entity: E,
        discrim: K,
    ) -> Option<Self::Get<'_>>;

    /// Return value of [`get_all`](Self::get_all).
    type GetAll<'t>: Iterator<Item = (<C as comp::Isotope<A>>::Discrim, &'t C)> + 't
    where
        Self: 't;
    /// Iterates over all isotopes of the component type for the given entity.
    ///
    /// The yielded discriminants are not in any guaranteed order.
    fn get_all<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Self::GetAll<'_>;

    // /// Return value of [`with`](Self::with).
    // type With<'t>: SpecificWriteIsotope<A, C>;
    // /// Creates an accessor with fixed discriminant.
    // fn with(&self, discrim: C::Discrim) -> Self::With<'_>;
}

/// Provides access to an isotope component in a specific archetype.
pub trait WriteIsotope<A: Archetype, C: comp::Isotope<A>>: ReadIsotope<A, C> {
    /// Returns a mutable reference to the component for the specified entity and discriminant,
    /// automatically initialized with the default initializer if present,
    /// or `None` if the component is unset and has no default initializer.
    ///
    /// Note that this method returns `Option<&mut C>`, not `&mut Option<C>`.
    /// This means setting the Option itself to `Some`/`None` will not modify any stored value.
    /// Use [`WriteIsotope::set`] to add/remove a component.
    fn try_get_mut<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        discrim: C::Discrim,
    ) -> Option<&mut C>;

    /// Returns a mutable reference to the component for the specified entity and discriminant.
    ///
    /// This method is infallible, assuming [`comp::Must`] is only implemented
    /// for components with [`Default`](comp::IsotopeInitStrategy::Default) init strategy.
    fn get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E, discrim: C::Discrim) -> &mut C
    where
        C: comp::Must<A>,
    {
        match self.try_get_mut(entity, discrim) {
            Some(comp) => comp,
            None => panic!(
                "Component {}/{} implements comp::Must but does not have a default initializer",
                any::type_name::<A>(),
                any::type_name::<C>(),
            ),
        }
    }

    /// Overwrites the component for the specified entity and discriminant.
    ///
    /// Passing `None` to this method removes the component from the entity.
    /// A subsequent call to `try_get_mut` would still return `Some`
    /// if the component uses [`Default`](comp::IsotopeInitStrategy::Default) init strategy.
    fn set<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        discrim: C::Discrim,
        value: Option<C>,
    ) -> Option<C>;

    // /// Return value of [`with`](Self::with).
    // type WithMut<'t>: SpecificWriteIsotope<A, C>;
    // /// Creates an accessor with fixed discriminant.
    // fn with_mut(&self, discrim: C::Discrim) -> Self::WithMut<'_>;
}

/// A [`WriteIsotope`] for a single specific discriminant.
pub trait SpecificWriteIsotope<A: Archetype, C: comp::Isotope<A>> {}
