use std::collections::BTreeSet;
use std::num::NonZeroU32;

use super::{BTreeHint, Ealloc, StaticShardAssigner};
use crate::test_util;

type BTree = super::Recycling<NonZeroU32, BTreeSet<NonZeroU32>, StaticShardAssigner, 2>;

#[test]
fn test_realloc_freed() {
    test_util::init();

    let mut ealloc = BTree::new(3);

    // use the first shard, which allocated block 1..3
    ealloc.shard_assigner.allocating_shard = 0;

    let alloc1: Vec<_> = (0..5).map(|_| ealloc.allocate(BTreeHint::default())).collect();
    log::trace!("allocated {alloc1:?}");

    assert_eq!(alloc1[0].get(), 1, "Shard 0 should allocate block 1..3");
    assert_eq!(alloc1[1].get(), 2, "Shard 0 should allocate block 1..3");
    assert_eq!(
        alloc1[2].get(),
        7,
        "Shard 0 should reallocate block 7..9 because shards 1,2 allocated blocks 3..7",
    );
    assert_eq!(
        alloc1[3].get(),
        8,
        "Shard 0 should reallocate block 7..9 because shards 1,2 allocated blocks 3..7",
    );
    assert_eq!(
        alloc1[4].get(),
        9,
        "Shard 0 should reallocate block 9..11 because shards 1,2 are still using blocks 3..7",
    );

    for &id in &alloc1 {
        ealloc.queue_deallocate(id);
    }
    ealloc.flush_deallocate();
    log::trace!("deallocated all");

    // expected similar result as test_distribute_sorted
    assert_eq!(
        ealloc.offline_shard(0).recycler.iter().copied().collect::<Vec<_>>(),
        vec![alloc1[0]],
    );
    assert_eq!(
        ealloc.offline_shard(1).recycler.iter().copied().collect::<Vec<_>>(),
        vec![alloc1[1], alloc1[2]],
    );
    assert_eq!(
        ealloc.offline_shard(2).recycler.iter().copied().collect::<Vec<_>>(),
        vec![alloc1[3], alloc1[4]],
    );

    // now we switch to shard 1 by default
    ealloc.shard_assigner.allocating_shard = 1;

    let alloc2: Vec<_> = (0..5).map(|_| ealloc.allocate(BTreeHint::default())).collect();
    log::trace!("allocated {alloc2:?}");

    assert!(
        ealloc.offline_shard(1).recycler.is_empty(),
        "alloc2[0..2] should be allocated from recycler",
    );
    assert_eq!(alloc2[0], alloc1[1], "alloc2[0..2] should be allocated from recycler");
    assert_eq!(alloc2[1], alloc1[2], "alloc2[0..2] should be allocated from recycler");

    assert_eq!(alloc2[2].get(), 3, "alloc2[2..4] should be allocated from the current 3.5 block",);
    assert_eq!(alloc2[3].get(), 4, "alloc2[2..4] should be allocated from the current 3.5 block",);
    assert_eq!(alloc2[4].get(), 11, "alloc2[3..5] should be allocated from the new 11..13 block",);
}
