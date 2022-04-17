use std::any::TypeId;

use super::sealed;
use crate::Archetype;

/// A type that may own entity references (no matter strong or weak).
pub trait Referrer {
    /// Executes the given function for each entity reference.
    fn visit_each<'s, F: Visitor<'s>>(&'s mut self, archetype: TypeId, visitor: &mut F);
}

pub trait Visitor<'s> {
    fn visit(&mut self, raw: sealed::RefMutRaw<'s>);
}

impl<'s, F: FnMut(&'s mut super::Raw)> Visitor<'s> for F {
    fn visit(&mut self, raw: sealed::RefMutRaw<'s>) { self(raw.0) }
}

impl<A: Archetype> Referrer for super::Weak<A> {
    fn visit_each<'s, F: Visitor<'s>>(&'s mut self, ty: TypeId, visitor: &mut F) {
        if ty == TypeId::of::<A>() {
            visitor.visit(sealed::RefMutRaw(&mut self.id));
        }
    }
}

impl<A: Archetype> Referrer for super::Entity<A> {
    fn visit_each<'s, F: Visitor<'s>>(&'s mut self, ty: TypeId, visitor: &mut F) {
        if ty == TypeId::of::<A>() {
            visitor.visit(sealed::RefMutRaw(&mut self.id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Visitor;

    // assert that `Visitor<'s>` is object-safe.
    fn _accepts_visitor<'s>(_object: &dyn Visitor<'s>) {}
}
