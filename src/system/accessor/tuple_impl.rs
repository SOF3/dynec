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
            unsafe fn entity<'this, 'e, 'ret>(this: &'this mut Self, #[allow(unused_variables)] entity: entity::TempRef<'e, A>) -> Self::Entity<'ret>
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
            unsafe fn chunk<'this, 'e, 'ret>(this: &'this mut Self, #[allow(unused_variables)] chunk: entity::TempRefChunk<'e, A>) -> Self::Chunk<'ret> {
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
    P0 p0, P1 p1,
    // P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20,
    // P21, P22, P23, P24, P25, P26, P27, P28, P29, P30, P31,
);
