use std::collections::BTreeSet;
use std::num::NonZeroU32;

use super::{distribute_sorted, BTreeHint, Ealloc};
use crate::entity::ealloc::StaticShardAssigner;
use crate::test_util;

#[test]
fn test_distribute_sorted_113367() {
    test_distribute_sorted(
        [1, 1, 3, 3, 6, 7],
        [
            (1, [1, 2, 3, 3, 6, 7]),
            (2, [2, 2, 3, 3, 6, 7]),
            (3, [2, 3, 3, 3, 6, 7]),
            (4, [3, 3, 3, 3, 6, 7]),
            (5, [3, 3, 3, 4, 6, 7]),
            (7, [3, 4, 4, 4, 6, 7]),
            (8, [4, 4, 4, 4, 6, 7]),
            (10, [4, 4, 5, 5, 6, 7]),
            (15, [5, 6, 6, 6, 6, 7]),
            (16, [6, 6, 6, 6, 6, 7]),
            (17, [6, 6, 6, 6, 7, 7]),
            (22, [7, 7, 7, 7, 7, 8]),
        ],
    );
}

#[test]
fn test_distribute_sorted_000() { test_distribute_sorted([0, 0, 0], [(5, [1, 2, 2])]); }

fn test_distribute_sorted<const N: usize>(
    sample: [usize; N],
    cases: impl IntoIterator<Item = (usize, [usize; N])>,
) {
    for (total, simulation) in cases {
        assert_eq!(sample.into_iter().sum::<usize>() + total, simulation.into_iter().sum()); // assert correctness of the test case

        let mut copy = sample;
        distribute_sorted(&mut copy, total);

        assert_eq!(copy, simulation);
    }
}

type BTree = super::Recycling<NonZeroU32, BTreeSet<NonZeroU32>, StaticShardAssigner>;

#[test]
fn test_realloc_freed() {
    test_util::init();

    let mut ealloc = BTree::new(3);

    // use the first shard, which allocated block 1..3
    ealloc.shard_assigner.allocating_shard = 0;

    let alloc1: Vec<_> = (0..5).map(|_| ealloc.allocate(BTreeHint::default())).collect();
    log::trace!("allocated {alloc1:?}");

    assert_eq!(alloc1[0].get(), 1, "Shards should allocate in order from global gauge");
    assert_eq!(alloc1[1].get(), 2, "Shards should allocate in order from global gauge");
    assert_eq!(alloc1[2].get(), 3, "Shards should allocate in order from global gauge");
    assert_eq!(alloc1[3].get(), 4, "Shards should allocate in order from global gauge");
    assert_eq!(alloc1[4].get(), 5, "Shards should allocate in order from global gauge");

    for &id in &alloc1 {
        ealloc.queue_deallocate(id);
    }

    ealloc.flush();
    log::trace!("deallocated all, ealloc state = {ealloc:?}");

    // this distribution is the same as test_distribute_sorted_000.
    assert_eq!(
        BTree::get_recycler_offline(&mut ealloc.recycler_shards, 0)
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![alloc1[0]],
    );
    assert_eq!(
        BTree::get_recycler_offline(&mut ealloc.recycler_shards, 1)
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![alloc1[1], alloc1[2]],
    );
    assert_eq!(
        BTree::get_recycler_offline(&mut ealloc.recycler_shards, 2)
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![alloc1[3], alloc1[4]],
    );

    assert_eq!(ealloc.recyclable.len(), 5, "recyclable should be refilled by queue_deallocate");

    // now we switch to shard 1 by default for offline allocation
    ealloc.shard_assigner.allocating_shard = 1;

    let alloc2: Vec<_> = (0..5).map(|_| ealloc.allocate(BTreeHint::default())).collect();
    log::trace!("allocated {alloc2:?} offline, ealloc state = {ealloc:?}");

    assert!(
        BTree::get_recycler_offline(&mut ealloc.recycler_shards, 1).is_empty(),
        "alloc2[0..2] should be allocated from recycler",
    );
    assert_eq!(alloc2[0], alloc1[1], "alloc2[0..2] should be allocated from recycler");
    assert_eq!(alloc2[1], alloc1[2], "alloc2[0..2] should be allocated from recycler");

    assert_eq!(alloc2[2].get(), 6, "alloc2[2..4] should be allocated from global gauge");
    assert_eq!(alloc2[3].get(), 7, "alloc2[2..4] should be allocated from global gauge");
    assert_eq!(alloc2[4].get(), 8, "alloc2[3..5] should be allocated from global gauge");

    assert_eq!(
        &BTree::get_reuse_queue_offline(&mut ealloc.reuse_queue_shards, 1)[..],
        &alloc2[0..2],
        "the first two allocations should be pushed to reuse queue"
    );
    assert_eq!(ealloc.recyclable.len(), 5, "recyclable is not refilled until flush");

    ealloc.flush();
    log::trace!("flushed after reallocation, ealloc state = {ealloc:?}");
    assert!(BTree::get_reuse_queue_offline(&mut ealloc.reuse_queue_shards, 1).is_empty());
    assert_eq!(ealloc.recyclable.len(), 3, "recyclable is drained after flush");

    let allocated_chunks: Vec<_> = ealloc.iter_allocated_chunks_offline().collect();
    assert_eq!(
        allocated_chunks,
        vec![
            NonZeroU32::new(2).expect("2 != 0")..NonZeroU32::new(4).expect("4 != 0"),
            NonZeroU32::new(6).expect("6 != 0")..NonZeroU32::new(9).expect("9 != 0"),
        ]
    );
}
