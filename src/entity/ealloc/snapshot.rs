use std::collections::BTreeSet;
use std::sync::Arc;
use std::{iter, ops};

use itertools::Itertools;

use super::iter_gaps;
use crate::entity::raw::Atomic as _;
use crate::entity::Raw;

// TODO change this into a trait to allow non-recycling ealloc.
// TODO make this a trait so that offline access does not need to clone the entire recyclable set.
/// A snapshot of the allocated entities during offline.
#[derive(Clone)]
pub struct Snapshot<E> {
    pub(super) gauge:      E,
    pub(super) recyclable: Arc<BTreeSet<E>>,
}

impl<E: Raw> Snapshot<E> {
    /// Iterates over all chunks of allocated entities.
    pub fn iter_allocated_chunks(&self) -> impl iter::FusedIterator<Item = ops::Range<E>> + '_ {
        iter_gaps(self.gauge, self.recyclable.iter().copied())
    }

    pub(crate) fn as_slice(&self) -> Slice<'_, E> {
        Slice {
            start:      E::new().load_mut(),
            end:        self.gauge,
            recyclable: &self.recyclable,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Slice<'t, E> {
    pub(crate) start:      E,
    pub(crate) end:        E,
    pub(crate) recyclable: &'t BTreeSet<E>,
}

impl<'t, E: Raw> Slice<'t, E> {
    pub(crate) fn midpoint_for_split(self) -> Option<E> {
        // TODO implement the algorithm in https://cs.stackexchange.com/q/155747/56834
        // when we find an implementation of BTreeSet that has order statistics.
        // For now, we just take the assumption that the holes are uniformly distributed.

        let midpt = self.start.approx_midpoint(self.end);
        let is_far = self.end.sub(midpt) < 8;
        is_far.then_some(midpt)
    }

    pub(crate) fn split_at(self, midpt: E) -> (Self, Self) {
        (
            Self { start: self.start, end: midpt, recyclable: self.recyclable },
            Self { start: midpt, end: self.end, recyclable: self.recyclable },
        )
    }

    pub(crate) fn split(self) -> (Self, Option<Self>) {
        let Some(midpt) = self.midpoint_for_split() else { return (self, None) };

        let (left, right) = self.split_at(midpt);
        (left, Some(right))
    }

    #[auto_enums::auto_enum(Iterator, marker = auto_enum_marker)]
    pub(crate) fn iter_chunks(self) -> impl Iterator<Item = ops::Range<E>> + 't {
        let first = match self.recyclable.first() {
            Some(&first) => first,
            None => return iter::once(self.start..self.end),
        };

        let gaps = self.recyclable.range(self.start..self.end).copied().chain(iter::once(self.end));
        let pairs = gaps.tuple_windows();
        let ret = iter::once(self.start..first)
            .chain(pairs.map(|(start, end)| (start.add(1))..end))
            .filter(|range| range.start != range.end);
        auto_enum_marker!(ret)
    }
}
