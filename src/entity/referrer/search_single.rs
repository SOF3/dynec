use super::{VisitMutArg, VisitStrongArgs, VisitStrongResult, VisitWeakArgs, VisitWeakResult};
use crate::util::DbgTypeId;

pub(crate) struct SearchSingleStrong {
    ty:               DbgTypeId,
    raw:              usize,
    pub(crate) found: usize,
}

impl SearchSingleStrong {
    pub(crate) fn new(ty: DbgTypeId, raw: usize) -> Self { Self { ty, raw, found: 0 } }
}

impl super::sealed::Sealed for SearchSingleStrong {}
impl VisitMutArg for SearchSingleStrong {
    #[inline]
    fn _visit_strong(&mut self, args: VisitStrongArgs) -> VisitStrongResult {
        if args.archetype == self.ty && args.raw == self.raw {
            self.found += 1;
        }
        VisitStrongResult { new_raw: args.raw }
    }

    #[inline]
    fn _visit_weak(&mut self, args: VisitWeakArgs) -> VisitWeakResult {
        VisitWeakResult { new_raw: args.raw }
    }
}
