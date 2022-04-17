#![cfg(test)]

use crate::Archetype;

pub(crate) enum TestArch {}
impl Archetype for TestArch {}

macro_rules! impl_test_simple_component {
    ($comp:ty, presence($($presence:tt)*), init($($init:tt)*), finalizer($finalizer:expr), entity_refs($($entity_ref_fields:ident),*)) => {
        impl crate::component::Simple<crate::test_util::TestArch> for $comp {
            const PRESENCE: crate::component::SimplePresence = impl_test_simple_component!(@presence $($presence)*);
            const INIT_STRATEGY: crate::component::SimpleInitStrategy<TestArch, Self> = impl_test_simple_component!(@init $($init)*);
            const IS_FINALIZER: bool = $finalizer;
        }

        impl crate::entity::Referrer for $comp {
            fn visit_each<'s, F: crate::entity::Visitor<'s>>(&'s mut self, archetype: TypeId, visitor: &mut F) {
                $(
                    crate::entity::Referrer::visit_each(&mut self.$entity_ref_fields, archetype, visitor);
                )*
            }
        }
    };
    (@presence Optional) => {
        crate::component::SimplePresence::Optional
    };
    (@presence Required) => {
        crate::component::SimplePresence::Required
    };
    (@init None) => {
        crate::component::SimpleInitStrategy::None
    };
    (@init Auto($expr:expr)) => {
        crate::component::SimpleInitStrategy::Auto($expr)
    };
}
