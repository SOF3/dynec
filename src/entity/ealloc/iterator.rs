use std::iter::FusedIterator;
use std::ops;
use std::sync::Arc;

use rayon::prelude::ParallelIterator;

use crate::entity::raw::Atomic;
use crate::entity::Raw;

pub(super) fn iter_gaps<'t, E: Raw>(
    gauge: E,
    breakpoints: impl Iterator<Item = E> + 't,
) -> impl Iterator<Item = ops::Range<E>> + FusedIterator + 't {
    IterGaps { gauge, breakpoints, previous: Previous::Initial }
        .filter(|range| range.start != range.end)
}

enum Previous<E: Raw> {
    Initial,
    Breakpoint(E),
    Finalized,
}
struct IterGaps<E: Raw, I: Iterator> {
    gauge:       E,
    breakpoints: I,
    previous:    Previous<E>,
}
impl<E: Raw, I: Iterator<Item = E>> Iterator for IterGaps<E, I> {
    type Item = ops::Range<E>;

    fn next(&mut self) -> Option<ops::Range<E>> {
        let start = match self.previous {
            Previous::Initial => E::new().load_mut(),
            Previous::Breakpoint(previous) => previous.add(1),
            Previous::Finalized => return None,
        };
        let (previous, end) = match self.breakpoints.next() {
            None => (Previous::Finalized, self.gauge),
            Some(breakpoint) => (Previous::Breakpoint(breakpoint), breakpoint),
        };
        self.previous = previous;
        Some(start..end)
    }
}
impl<E: Raw, I: Iterator<Item = E>> FusedIterator for IterGaps<E, I> {}

pub(super) fn par_iter_gaps<'t, E: Raw>(
    gauge: E,
    breakpoints: impl Iterator<Item = E> + 't,
) -> impl ParallelIterator<Item = ops::Range<E>> + 't {
    let gaps: Arc<[_]> = iter_gaps(gauge, breakpoints)
        .scan(0, |state, gap| {
            *state += gap.end.to_primitive() - gap.start.to_primitive();
            Some(IndexedGap { gap, inclusive_sum: *state })
        })
        .collect();

    let total_sum = gaps.last().map_or(0, |ig| ig.inclusive_sum);

    rayon::iter::split(Split { gaps, start: 0, end: total_sum }, Split::split)
        .flat_map_iter(|split| split.iter())
}

#[derive(Clone)]
struct IndexedGap<E: Raw> {
    gap:           ops::Range<E>,
    /// The prefix sum including this gap.
    inclusive_sum: usize,
}

impl<E: Raw> IndexedGap<E> {
    fn length(&self) -> usize { self.gap.end.to_primitive() - self.gap.start.to_primitive() }

    /// The prefix sum excluding this gap.
    fn exclusive_sum(&self) -> usize { self.inclusive_sum - self.length() }

    fn restrict(&self, start_sum: usize, end_sum: usize) -> Option<ops::Range<E>> {
        let mut start_entity = self.gap.start.to_primitive();
        let mut end_entity = self.gap.end.to_primitive();

        if start_sum >= self.inclusive_sum {
            // starts after this gap
            // the equality case means their start is at our exclusive end
            return None;
        }
        if self.exclusive_sum() < start_sum {
            // should start later
            // start_sum < inclusive_sum
            // <=> start_sum < exclusive_sum + length
            // <=> start_entity + start_sum - exclusive_sum < end_entity
            start_entity += start_sum - self.exclusive_sum();
        }

        if end_sum <= self.exclusive_sum() {
            // ends before this gap
            // the equality case means our start is their exclusive end
            return None;
        }
        if self.inclusive_sum > end_sum {
            // should end earlier
            // end_sum > exclusive_sum
            // <=> end_sum > inclusive_sum - length
            // <=> end_sum > inclusive_sum - end_entity + start_entity
            // <=> end_entity - (inclusive_sum - end_sum) > start_entity
            end_entity -= self.inclusive_sum - end_sum;
        }

        Some(E::from_primitive(start_entity)..E::from_primitive(end_entity))
    }
}

fn index_gaps<E: Raw>(gaps: impl Iterator<Item = ops::Range<E>>) -> Arc<[IndexedGap<E>]> {
    gaps.scan(0, |prefix_sum, gap| {
        *prefix_sum += gap.end.to_primitive() - gap.start.to_primitive();
        Some(IndexedGap { gap, inclusive_sum: *prefix_sum })
    })
    .collect()
}

#[derive(Clone)]
struct Split<E: Raw> {
    gaps:  Arc<[IndexedGap<E>]>,
    start: usize,
    end:   usize,
}
impl<E: Raw> Split<E> {
    fn split(self) -> (Self, Option<Self>) {
        let Self { start, end, .. } = self;

        let midpoint = start + (end - start) / 2;
        if midpoint == start || midpoint == end {
            return (self, None);
        }

        (Split { end: midpoint, ..self.clone() }, Some(Split { start: midpoint, ..self }))
    }

    fn iter(self) -> impl Iterator<Item = ops::Range<E>> {
        // Out of an abundance of caution, let's include one or two extra gaps here.
        let start_gap = self.gaps.partition_point(|gap| gap.inclusive_sum < self.start); // make the predicate more strict
        let end_gap = self.gaps.partition_point(|gap| gap.exclusive_sum() <= self.end); // make the predicate less strict
        ArcSliceClonedIter { arc: self.gaps, index: start_gap, until: end_gap }
            .filter_map(move |gap| gap.restrict(self.start, self.end))
    }
}

struct ArcSliceClonedIter<T> {
    arc:   Arc<[T]>,
    index: usize,
    until: usize,
}

impl<T: Clone> Iterator for ArcSliceClonedIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        if index >= self.until {
            return None;
        }
        self.index += 1;
        self.arc.get(index).cloned()
    }
}
