//! Implement [`super::referrer::Referrer`] for standard types.

use std::{collections, hash};

use super::*;

impl<T: Referrer> Referrer for Option<T> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            T::visit_type(arg);
        }
    }

    fn visit_mut<U: VisitMutArg>(&mut self, arg: &mut U) {
        if let Some(value) = self {
            value.visit_mut(arg);
        }
    }
}

impl<T: Referrer> Referrer for Box<T> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            T::visit_type(arg);
        }
    }

    fn visit_mut<U: VisitMutArg>(&mut self, arg: &mut U) { T::visit_mut(&mut **self, arg); }
}

impl<T: Referrer> Referrer for Vec<T> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            T::visit_type(arg);
        }
    }

    fn visit_mut<U: VisitMutArg>(&mut self, arg: &mut U) {
        self.iter_mut().for_each(|value| value.visit_mut(arg))
    }
}

impl<T: Referrer> Referrer for collections::VecDeque<T> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            T::visit_type(arg);
        }
    }

    fn visit_mut<U: VisitMutArg>(&mut self, arg: &mut U) {
        self.iter_mut().for_each(|value| value.visit_mut(arg))
    }
}

// for whatever reason someone wants to use a linked list...
impl<T: Referrer> Referrer for collections::LinkedList<T> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            T::visit_type(arg);
        }
    }

    fn visit_mut<U: VisitMutArg>(&mut self, arg: &mut U) {
        self.iter_mut().for_each(|value| value.visit_mut(arg))
    }
}

impl<T: Referrer, const N: usize> Referrer for [T; N] {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            T::visit_type(arg);
        }
    }

    fn visit_mut<U: VisitMutArg>(&mut self, arg: &mut U) {
        self.iter_mut().for_each(|value| value.visit_mut(arg))
    }
}

impl<K: Eq + Ord + 'static, V: Referrer> Referrer for collections::BTreeMap<K, V> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            V::visit_type(arg);
        }
    }

    fn visit_mut<U: VisitMutArg>(&mut self, arg: &mut U) {
        self.values_mut().for_each(|value| value.visit_mut(arg))
    }
}

impl<K: Eq + hash::Hash + 'static, V: Referrer> Referrer for collections::HashMap<K, V> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_continue() {
            V::visit_type(arg);
        }
    }

    fn visit_mut<U: VisitMutArg>(&mut self, arg: &mut U) {
        self.values_mut().for_each(|value| value.visit_mut(arg))
    }
}

// no implementation for Arc because we cannot guarantee each is only called once.

// no implementation for PhantomData because it is unclear what the actual intention is.

// no implementation for tuples because they are usually not all Referrer.
