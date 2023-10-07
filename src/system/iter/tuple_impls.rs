#![allow(non_snake_case, clippy::unused_unit)]

use super::{IntoZip, Zip, ZipChunked};
use crate::{entity, Archetype};

macro_rules! impl_zip_for_tuple {
    ($($idents:ident)*) => {
        impl<A: Archetype, $($idents,)* > IntoZip<A> for ($($idents,)*)
        where
            $($idents: IntoZip<A>,)*
        {
            type IntoZip = ($(
                <$idents as IntoZip<A>>::IntoZip,
            )*);

            fn into_zip(self) -> Self::IntoZip {
                let ($($idents,)*) = self;
                ($(
                    IntoZip::<A>::into_zip($idents),
                )*)
            }
        }

        impl<A: Archetype, $($idents,)* > Zip<A> for ($($idents,)*)
        where
            $($idents: Zip<A>,)*
        {
            fn split(&mut self, offset: A::RawEntity) -> Self {
                let ($($idents,)*) = self;
                ($(
                    Zip::<A>::split($idents, offset),
                )*)
            }

            type Item = ($(
                <$idents as Zip<A>>::Item,
            )*);
            fn get<E: entity::Ref<Archetype = A>>(self, entity: E) -> Self::Item {
                let ($($idents,)*) = self;
                let entity = entity::TempRef::<A>::new(entity.id());
                ($(
                    Zip::<A>::get($idents, entity),
                )*)
            }
        }

        impl<A: Archetype, $($idents,)* > ZipChunked<A> for ($($idents,)*)
        where
            $($idents: ZipChunked<A>,)*
        {
            type Chunk = ($(
                <$idents as ZipChunked<A>>::Chunk,
            )*);
            fn get_chunk(self, chunk: entity::TempRefChunk<A>) -> Self::Chunk {
                let ($($idents,)*) = self;
                ($(
                    ZipChunked::<A>::get_chunk($idents, chunk),
                )*)
            }
        }
    }
}

macro_rules! impl_zip_for_tuple_accumulate {
    ($feature:literal $first:ident $($rest:tt)*) => {
        impl_zip_for_tuple_accumulate!($feature $($rest)*);
        #[cfg(feature = $feature)]
        impl_zip_for_tuple_accumulate!(@MIXED $first $($rest)*);
    };
    ($outer_feature:literal $inner_feature:literal $($rest:tt)*) => {
        impl_zip_for_tuple_accumulate!($inner_feature $($rest)*);
    };
    ($outer_feature:literal @ALWAYS $($rest:tt)*) => {
        impl_zip_for_tuple_accumulate!(@ALWAYS $($rest)*);
    };
    (@ALWAYS $first:ident $($rest:tt)*) => {
        impl_zip_for_tuple_accumulate!(@ALWAYS $($rest)*);
        impl_zip_for_tuple!($first $($rest)*);
    };
    (@ALWAYS) => {
        #[allow(unused_variables)]
        const _: () = {
            impl_zip_for_tuple!();
        };
    };
    (@MIXED $($idents_front:ident)* $($feature:literal $($idents_feature:ident)*)* @ALWAYS $($idents_always:ident)*) => {
        impl_zip_for_tuple!($($idents_front)* $($($idents_feature)*)* $($idents_always)*);
    };
}

impl_zip_for_tuple_accumulate!(
    "tuple-impl-32-zip" T1 T2 T3 T4 T5 T6 T7 T8
    "tuple-impl-24-zip" T9 T10 T11 T12 T13 T14 T15 T16
    "tuple-impl-16-zip" T17 T18 T19 T20 T21 T22 T23 T24
    "tuple-impl-8-zip" T25 T26 T27 T28
    @ALWAYS T29 T30 T31 T32
);
