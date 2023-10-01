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
    () => {
        #[allow(unused_variables)]
        const _: () = {
            impl_zip_for_tuple!();
        };
    };
    ($first:ident $($rest:ident)*) => {
        impl_zip_for_tuple_accumulate!($($rest)*);
        impl_zip_for_tuple!($first $($rest)*);
    }
}

impl_zip_for_tuple_accumulate!(
    P1 P2 P3 P4 P5 P6 P7 P8 P9 P10 P11 P12 P13 P14 P15 P16
    P17 P18 P19 P20 P21 P22 P23 P24 P25 P26 P27 P28 P29 P30 P31 P32
);
