use crate::system::accessor::{self, Accessor};
use crate::{entity, Archetype};

macro_rules! impl_accessor_set_for_tuple {
    ($($ty:ident),* $(,)?) => {
        impl<A: Archetype, $($ty,)*> accessor::Set<A> for ($($ty,)*)
        where $(
            $ty: Accessor<A>,
        )*
        {
            type Entity<'t> = ($(<$ty as Accessor<A>>::Entity<'t>,)*)
            where
                Self: 't;
            fn project_entity<'e>(&mut self, #[allow(unused_variables)] entity: entity::TempRef<'e, A>) -> Self::Entity<'_> {
                #[allow(non_snake_case)]
                let ($($ty,)*) = self;
                #[allow(clippy::unused_unit)]
                {
                    (
                        $(<$ty as Accessor<A>>::entity($ty, entity),)*
                    )
                }
            }

            type Chunk<'t> = ($(<$ty as Accessor<A>>::Chunk<'t>,)*)
            where
                Self: 't;
            fn project_chunk<'e>(&mut self, #[allow(unused_variables)] chunk: entity::TempRefChunk<'e, A>) -> Self::Chunk<'_> {
                #[allow(non_snake_case)]
                let ($($ty,)*) = self;

                #[allow(clippy::unused_unit)]
                {
                    (
                        $(<$ty as Accessor<A>>::chunk($ty, chunk),)*
                    )
                }
            }
        }
    }
}

macro_rules! impl_accessor_set_for_tuple_accumulate {
    () => {
        impl_accessor_set_for_tuple!();
    };
    ($first:ident $(, $rest:ident)* $(,)?) => {
        impl_accessor_set_for_tuple_accumulate!($($rest),*);
        impl_accessor_set_for_tuple!($first $(, $rest)*);
    }
}
impl_accessor_set_for_tuple_accumulate!(
    P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20,
    P21, P22, P23, P24, P25, P26, P27, P28, P29, P30, P31,
);
