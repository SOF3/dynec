use std::{collections, hash};

use super::*;

impl<T: Referrer> Dyn for Option<T> {
    fn visit(&mut self, arg: &mut VisitArg) {
        if let Some(this) = self {
            this.visit(arg);
        }
    }
}
impl<T: Referrer> Referrer for Option<T> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            T::visit_type(arg);
        }
    }
}

impl<T: Referrer> Dyn for Box<T> {
    fn visit(&mut self, arg: &mut VisitArg) {
        let this: &mut T = self;
        this.visit(arg);
    }
}
impl<T: Referrer> Referrer for Box<T> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            T::visit_type(arg);
        }
    }
}

// Rc and Arc are not implemented because we cannot guarantee each is only called once.

impl<T: Referrer> Dyn for Vec<T> {
    fn visit(&mut self, arg: &mut VisitArg) {
        for item in self {
            item.visit(arg);
        }
    }
}
impl<T: Referrer> Referrer for Vec<T> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            T::visit_type(arg);
        }
    }
}

impl<T: Referrer> Dyn for collections::VecDeque<T> {
    fn visit(&mut self, arg: &mut VisitArg) {
        for item in self {
            item.visit(arg);
        }
    }
}
impl<T: Referrer> Referrer for collections::VecDeque<T> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            T::visit_type(arg);
        }
    }
}

impl<T: Referrer> Dyn for collections::LinkedList<T> {
    fn visit(&mut self, arg: &mut VisitArg) {
        for item in self {
            item.visit(arg);
        }
    }
}
impl<T: Referrer> Referrer for collections::LinkedList<T> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            T::visit_type(arg);
        }
    }
}

impl<T: Referrer, const N: usize> Dyn for [T; N] {
    fn visit(&mut self, arg: &mut VisitArg) {
        for item in self {
            item.visit(arg);
        }
    }
}
impl<T: Referrer, const N: usize> Referrer for [T; N] {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            T::visit_type(arg);
        }
    }
}

impl<K: Eq + Ord + 'static, V: Referrer> Dyn for collections::BTreeMap<K, V> {
    fn visit(&mut self, arg: &mut VisitArg) {
        for item in self.values_mut() {
            item.visit(arg);
        }
    }
}
impl<K: Eq + Ord + 'static, V: Referrer> Referrer for collections::BTreeMap<K, V> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            V::visit_type(arg);
        }
    }
}

impl<K: Eq + hash::Hash + 'static, V: Referrer> Dyn for collections::HashMap<K, V> {
    fn visit(&mut self, arg: &mut VisitArg) {
        for item in self.values_mut() {
            item.visit(arg);
        }
    }
}
impl<K: Eq + hash::Hash + 'static, V: Referrer> Referrer for collections::HashMap<K, V> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            V::visit_type(arg);
        }
    }
}

impl<T: Referrer> Dyn for PhantomData<T> {
    fn visit(&mut self, arg: &mut VisitArg) {}
}
impl<T: Referrer> Referrer for PhantomData<T> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            // phantom data should work as if the type actually exists
            T::visit_type(arg);
        }
    }
}

// tuples are not implemented because they are usually not all Referrer.
