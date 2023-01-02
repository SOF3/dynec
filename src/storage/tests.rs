use std::marker::PhantomData;
use std::num::NonZeroU32;

use crate::storage::Partition;
use crate::Storage;

macro_rules! test_storage {
    ($ident:ident $storage:ty) => {
        crate::storage::tests::test_storage! { @$ident $storage =>
            test_single_small_hole
            test_single_big_hole_with_reinsertion
            test_partition_no_panic
            #[should_panic = "Entity 5 is not in the partition ..4"] test_partition_panic_left_some
            #[should_panic = "Entity 4 is not in the partition ..4"] test_partition_panic_left_none
            #[should_panic = "Entity 3 is not in the partition 5.."] test_partition_panic_right_some
            #[should_panic = "Entity 4 is not in the partition 5.."] test_partition_panic_right_none
            #[should_panic = "Entity 4 is not in the partition ..3"] test_repartition_panic_ll_lr
            #[should_panic = "Entity 8 is not in the partition ..3"] test_repartition_panic_ll_r
            #[should_panic = "Entity 2 is not in the partition 3.."] test_repartition_panic_lr_ll
            #[should_panic = "Entity 8 is not in the partition ..5"] test_repartition_panic_lr_r
            #[should_panic = "Entity 2 is not in the partition 5.."] test_repartition_panic_rl_l
            #[should_panic = "Entity 8 is not in the partition ..7"] test_repartition_panic_rl_rr
            #[should_panic = "Entity 3 is not in the partition 7.."] test_repartition_panic_rr_l
            #[should_panic = "Entity 6 is not in the partition 7.."] test_repartition_panic_rr_rl
        }
    };
    (@CHUNKED $storage:ty => $($(#[$meta:meta])* $tests:ident)*) => {
        $(
            $(#[$meta])*
            #[test]
            fn $tests() {
                crate::storage::tests::$tests::<$storage, crate::storage::tests::RealChunker<$storage>>();
            }
        )*
    };
    (@NON_CHUNKED $storage:ty => $($(#[$meta:meta])* $tests:ident)*) => {
        $(
            $(#[$meta])*
            #[test]
            fn $tests() {
                crate::storage::tests::$tests::<$storage, crate::storage::tests::FakeChunker>();
            }
        )*
    }
}

pub(crate) use test_storage;

pub(super) trait Chunker<S> {
    fn to_chunks(s: &S) -> Option<Vec<(u32, Vec<i64>)>>;
    fn to_chunks_mut(s: &mut S) -> Option<Vec<(u32, Vec<i64>)>>;
}

pub(super) struct RealChunker<S>(PhantomData<S>);
impl<S: Storage<RawEntity = NonZeroU32, Comp = i64>> Chunker<S> for RealChunker<S> {
    fn to_chunks(s: &S) -> Option<Vec<(u32, Vec<i64>)>> {
        Some(s.iter_chunks().map(|chunk| (chunk.start.get(), chunk.slice.to_vec())).collect())
    }
    fn to_chunks_mut(s: &mut S) -> Option<Vec<(u32, Vec<i64>)>> {
        Some(s.iter_chunks_mut().map(|chunk| (chunk.start.get(), chunk.slice.to_vec())).collect())
    }
}

pub(super) struct FakeChunker;
impl<S> Chunker<S> for FakeChunker {
    fn to_chunks(_: &S) -> Option<Vec<(u32, Vec<i64>)>> { None }
    fn to_chunks_mut(_: &mut S) -> Option<Vec<(u32, Vec<i64>)>> { None }
}

pub(super) fn test_single_small_hole<S, P>()
where
    S: Storage<RawEntity = NonZeroU32, Comp = i64>,
    P: Chunker<S>,
{
    let mut storage = S::default();
    for i in 1..=10 {
        storage.set(NonZeroU32::new(i).unwrap(), Some(i64::from(i)));
    }

    for i in 1..=10 {
        assert_eq!(storage.get(NonZeroU32::new(i).unwrap()), Some(&i64::from(i)));
    }

    storage.set(NonZeroU32::new(3).unwrap(), None);
    for i in (1..3).chain(4..=10) {
        assert_eq!(storage.get(NonZeroU32::new(i).unwrap()), Some(&i64::from(i)));
    }

    let items: Vec<_> = storage.iter().map(|(entity, value)| (entity.get(), *value)).collect();
    assert_eq!(
        items,
        vec![(1, 1), (2, 2), (4, 4), (5, 5), (6, 6), (7, 7), (8, 8), (9, 9), (10, 10)]
    );

    let items: Vec<_> = storage.iter_mut().map(|(entity, value)| (entity.get(), *value)).collect();
    assert_eq!(
        items,
        vec![(1, 1), (2, 2), (4, 4), (5, 5), (6, 6), (7, 7), (8, 8), (9, 9), (10, 10)]
    );

    if let Some(chunks) = P::to_chunks(&storage) {
        assert_eq!(chunks, vec![(1, vec![1, 2]), (4, vec![4, 5, 6, 7, 8, 9, 10])]);
    }

    if let Some(chunks) = P::to_chunks_mut(&mut storage) {
        assert_eq!(chunks, vec![(1, vec![1, 2]), (4, vec![4, 5, 6, 7, 8, 9, 10])]);
    }
}

pub(super) fn test_single_big_hole_with_reinsertion<S, P>()
where
    S: Storage<RawEntity = NonZeroU32, Comp = i64>,
    P: Chunker<S>,
{
    let mut storage = S::default();
    for i in 1..=10 {
        storage.set(NonZeroU32::new(i).unwrap(), Some(i64::from(i)));
    }

    for i in 1..=10 {
        assert_eq!(storage.get(NonZeroU32::new(i).unwrap()), Some(&i64::from(i)));
    }

    for i in 3..6 {
        storage.set(NonZeroU32::new(i).unwrap(), None);
    }
    storage.set(NonZeroU32::new(4).unwrap(), Some(i64::from(4)));

    for i in (1..3).chain(6..=10) {
        assert_eq!(storage.get(NonZeroU32::new(i).unwrap()), Some(&i64::from(i)));
    }

    let items: Vec<_> = storage.iter().map(|(entity, value)| (entity.get(), *value)).collect();
    assert_eq!(items, vec![(1, 1), (2, 2), (4, 4), (6, 6), (7, 7), (8, 8), (9, 9), (10, 10)]);

    let items: Vec<_> = storage.iter_mut().map(|(entity, value)| (entity.get(), *value)).collect();
    assert_eq!(items, vec![(1, 1), (2, 2), (4, 4), (6, 6), (7, 7), (8, 8), (9, 9), (10, 10)]);

    if let Some(chunks) = P::to_chunks(&storage) {
        assert_eq!(chunks, vec![(1, vec![1, 2]), (4, vec![4]), (6, vec![6, 7, 8, 9, 10])]);
    }

    if let Some(chunks) = P::to_chunks_mut(&mut storage) {
        assert_eq!(chunks, vec![(1, vec![1, 2]), (4, vec![4]), (6, vec![6, 7, 8, 9, 10])]);
    }
}

/// Returns a storage containing entities 1,2,3,5,6,8,9 (without 4 and 7).
fn setup_partition_storage<S: Storage<RawEntity = NonZeroU32, Comp = i64>>() -> S {
    let mut storage = S::default();
    for i in [1, 2, 3, 5, 6, 8, 9] {
        storage.set(NonZeroU32::new(i).unwrap(), Some(i64::from(i)));
    }
    storage
}

pub(super) fn test_partition_no_panic<S, P>()
where
    S: Storage<RawEntity = NonZeroU32, Comp = i64>,
{
    let mut storage: S = setup_partition_storage();
    {
        let (mut left, mut right) = storage.partition_at(NonZeroU32::new(4).unwrap());
        assert_eq!(left.get_mut(NonZeroU32::new(1).unwrap()), Some(&mut 1));
        assert_eq!(left.get_mut(NonZeroU32::new(3).unwrap()), Some(&mut 3));
        assert_eq!(right.get_mut(NonZeroU32::new(4).unwrap()), None);
        assert_eq!(right.get_mut(NonZeroU32::new(5).unwrap()), Some(&mut 5));
        assert_eq!(right.get_mut(NonZeroU32::new(9).unwrap()), Some(&mut 9));
    }
    {
        let (mut left, mut right) = storage.partition_at(NonZeroU32::new(5).unwrap());
        assert_eq!(left.get_mut(NonZeroU32::new(1).unwrap()), Some(&mut 1));
        assert_eq!(left.get_mut(NonZeroU32::new(3).unwrap()), Some(&mut 3));
        assert_eq!(left.get_mut(NonZeroU32::new(4).unwrap()), None);
        assert_eq!(right.get_mut(NonZeroU32::new(5).unwrap()), Some(&mut 5));
        assert_eq!(right.get_mut(NonZeroU32::new(9).unwrap()), Some(&mut 9));
    }
}

pub(super) fn test_partition_panic_left_some<S, P>()
where
    S: Storage<RawEntity = NonZeroU32, Comp = i64>,
{
    let mut storage: S = setup_partition_storage();
    let (mut left, _) = storage.partition_at(NonZeroU32::new(4).unwrap());
    left.get_mut(NonZeroU32::new(5).unwrap());
}

pub(super) fn test_partition_panic_left_none<S, P>()
where
    S: Storage<RawEntity = NonZeroU32, Comp = i64>,
{
    let mut storage: S = setup_partition_storage();
    let (mut left, _) = storage.partition_at(NonZeroU32::new(4).unwrap());
    left.get_mut(NonZeroU32::new(4).unwrap());
}

pub(super) fn test_partition_panic_right_some<S, P>()
where
    S: Storage<RawEntity = NonZeroU32, Comp = i64>,
{
    let mut storage: S = setup_partition_storage();
    let (_, mut right) = storage.partition_at(NonZeroU32::new(5).unwrap());
    right.get_mut(NonZeroU32::new(3).unwrap());
}

pub(super) fn test_partition_panic_right_none<S, P>()
where
    S: Storage<RawEntity = NonZeroU32, Comp = i64>,
{
    let mut storage: S = setup_partition_storage();
    let (_, mut right) = storage.partition_at(NonZeroU32::new(5).unwrap());
    right.get_mut(NonZeroU32::new(4).unwrap());
}

macro_rules! repartition_panic_test {
    (
        $(
            $ident:ident {
                partition $first_cut:literal take $first_half:ident;
                partition $second_cut:literal take $second_half:ident;
                panic at get_mut($probe:literal);
            }
        )*
    ) => { $(
        pub(super) fn $ident<S, P>()
        where
            S: Storage<RawEntity = NonZeroU32, Comp = i64>,
        {
            let mut storage: S = setup_partition_storage();

            let first_pair = storage.partition_at(NonZeroU32::new($first_cut).unwrap());
            let mut first = repartition_panic_test!(@take $first_half of first_pair);

            let second_pair = first.partition_at(NonZeroU32::new($second_cut).unwrap());
            let mut second = repartition_panic_test!(@take $second_half of second_pair);

            second.get_mut(NonZeroU32::new($probe).unwrap());
        }
    )* };
    (@take left of $v:expr) => {
        ($v).0
    };
    (@take right of $v:expr) => {
        ($v).1
    };
}

repartition_panic_test! {
    test_repartition_panic_ll_lr {
        partition 5 take left;
        partition 3 take left;
        panic at get_mut(4);
    }
    test_repartition_panic_ll_r {
        partition 5 take left;
        partition 3 take left;
        panic at get_mut(8);
    }
    test_repartition_panic_lr_ll {
        partition 5 take left;
        partition 3 take right;
        panic at get_mut(2);
    }
    test_repartition_panic_lr_r {
        partition 5 take left;
        partition 3 take right;
        panic at get_mut(8);
    }
    test_repartition_panic_rl_l {
        partition 5 take right;
        partition 7 take left;
        panic at get_mut(2);
    }
    test_repartition_panic_rl_rr {
        partition 5 take right;
        partition 7 take left;
        panic at get_mut(8);
    }
    test_repartition_panic_rr_l {
        partition 5 take right;
        partition 7 take right;
        panic at get_mut(3);
    }
    test_repartition_panic_rr_rl {
        partition 5 take right;
        partition 7 take right;
        panic at get_mut(6);
    }
}
