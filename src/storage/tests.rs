use std::marker::PhantomData;
use std::num::NonZeroU32;

use crate::Storage;

macro_rules! test_storage {
    ($ident:ident $storage:ty) => {
        crate::storage::tests::test_storage! { @$ident $storage =>
            test_single_small_hole
            test_single_big_hole_with_reinsertion
        }
    };
    (@CHUNKED $storage:ty => $($tests:ident)*) => {
        $(
            #[test]
            fn $tests() {
                crate::storage::tests::$tests::<$storage, crate::storage::tests::RealChunker<$storage>>();
            }
        )*
    };
    (@NON_CHUNKED $storage:ty => $($tests:ident)*) => {
        $(
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
