use crate::system::accessor::{self, Accessor};
use crate::{entity, Archetype};

macro_rules! impl_accessor_set_for_tuple {
    ($($ty:ident $var:ident,)*) => {
        // Safety: accessor::Accessor documentation justified this.
        unsafe impl<A: Archetype, $($ty,)*> Accessor<A> for ($($ty,)*)
        where $(
            $ty: Accessor<A>,
        )*
        {
            type Entity<'t> = ($(<$ty as Accessor<A>>::Entity<'t>,)*)
            where
                Self: 't;
            unsafe fn entity<'ret>(this: &mut Self, #[allow(unused_variables)] entity: entity::TempRef<'_, A>) -> Self::Entity<'ret>
            {
                #[allow(non_snake_case)]
                let ($($var,)*) = this;
                #[allow(clippy::unused_unit)]
                {
                    (
                        $(<$ty as Accessor<A>>::entity($var, entity),)*
                    )
                }
            }
        }

        // Safety: accessor::Chunked documentation justified this.
        unsafe impl<A: Archetype, $($ty,)*> accessor::Chunked<A> for ($($ty,)*)
        where $(
            $ty: accessor::Chunked<A>,
        )*
        {
            type Chunk<'t> = ($(<$ty as accessor::Chunked<A>>::Chunk<'t>,)*)
            where
                Self: 't;
            unsafe fn chunk<'ret>(this: &mut Self, #[allow(unused_variables)] chunk: entity::TempRefChunk<'_, A>) -> Self::Chunk<'ret> {
                #[allow(non_snake_case)]
                let ($($var,)*) = this;

                #[allow(clippy::unused_unit)]
                {
                    (
                        $(<$ty as accessor::Chunked<A>>::chunk($var, chunk),)*
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
    ($first_ty:ident $first_var:ident, $($rest_ty:ident $rest_var:ident,)*) => {
        impl_accessor_set_for_tuple_accumulate!($($rest_ty $rest_var,)*);
        impl_accessor_set_for_tuple!($first_ty $first_var, $($rest_ty $rest_var,)*);
    }
}
impl_accessor_set_for_tuple_accumulate!(
    P0 p0, P1 p1, P2 p2, P3 p3, P4 p4, P5 p5, P6 p6, P7 p7, P8 p8, P9 p9, P10 p10, P11 p11, P12 p12, P13 p13, P14 p14, P15 p15, P16 p16, P17 p17, P18 p18, P19 p19, P20 p20, P21 p21, P22 p22, P23 p23, P24 p24, P25 p25, P26 p26, P27 p27, P28 p28, P29 p29, P30 p30, P31 p31,
);
