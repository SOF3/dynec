//! A storage that can switch between two implementations in offline mode.
//! See [`Mux`] for more information.

use std::marker::PhantomData;

use replace_with::replace_with_or_abort;

use super::Storage;
use crate::entity;

/// A storage that can switched to another storage.
pub trait Source: Storage {
    fn into_iter(self) -> Box<dyn Iterator<Item = (Self::RawEntity, Self::Comp)>>;
}

/// A storage that can switched from another storage.
pub trait Sink: Storage {
    fn from_iter(iter: Box<dyn Iterator<Item = (Self::RawEntity, Self::Comp)>>) -> Self;
}

/// A storage that can switch between two implementations in offline mode.
///
/// A mux is typically used for changing the implementation of a storage
/// when a certain condition is reached,
/// e.g. when the number of entries in the storage reaches a certain threshold.
pub enum Mux<E, C, P, Q> {
    /// The first backend.
    P(P, PhantomData<(E, C)>),
    /// The second backend.
    Q(Q),
}

impl<E, C, P, Q> Default for Mux<E, C, P, Q>
where
    P: Default,
{
    fn default() -> Self { Self::P(P::default(), PhantomData) }
}

impl<E: entity::Raw, C: Send + Sync + 'static, P, Q> Storage for Mux<E, C, P, Q>
where
    P: Storage<RawEntity = E, Comp = C>,
    Q: Storage<RawEntity = E, Comp = C>,
{
    type RawEntity = E;
    type Comp = C;

    fn get(&self, id: E) -> Option<&Self::Comp> {
        match self {
            Self::P(p, PhantomData) => p.get(id),
            Self::Q(q) => q.get(id),
        }
    }

    fn get_mut(&mut self, id: E) -> Option<&mut Self::Comp> {
        match self {
            Self::P(p, PhantomData) => p.get_mut(id),
            Self::Q(q) => q.get_mut(id),
        }
    }

    fn set(&mut self, id: E, value: Option<Self::Comp>) -> Option<Self::Comp> {
        match self {
            Self::P(p, PhantomData) => p.set(id, value),
            Self::Q(q) => q.set(id, value),
        }
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (E, &Self::Comp)> + '_> {
        match self {
            Self::P(p, PhantomData) => p.iter(),
            Self::Q(q) => q.iter(),
        }
    }

    fn iter_mut(&mut self) -> Box<dyn Iterator<Item = (E, &mut Self::Comp)> + '_> {
        match self {
            Self::P(p, PhantomData) => p.iter_mut(),
            Self::Q(q) => q.iter_mut(),
        }
    }
}

impl<E: entity::Raw, C: Send + Sync + 'static, P, Q> Mux<E, C, P, Q>
where
    P: Storage<RawEntity = E, Comp = C> + Sink,
    Q: Storage<RawEntity = E, Comp = C> + Source,
{
    pub fn set_p(&mut self) {
        replace_with_or_abort(self, |mux| match mux {
            Self::P(..) => mux,
            Self::Q(q) => {
                let iter = q.into_iter();
                Self::P(P::from_iter(iter), PhantomData)
            }
        })
    }
}

impl<E: entity::Raw, C: Send + Sync + 'static, P, Q> Mux<E, C, P, Q>
where
    P: Storage<RawEntity = E, Comp = C> + Source,
    Q: Storage<RawEntity = E, Comp = C> + Sink,
{
    pub fn set_q(&mut self) {
        replace_with_or_abort(self, |mux| match mux {
            Self::P(p, PhantomData) => {
                let iter = p.into_iter();
                Self::Q(Q::from_iter(iter))
            }
            Self::Q(_) => mux,
        })
    }
}
