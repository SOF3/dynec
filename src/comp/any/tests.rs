use super::*;
use crate::test_util::TestArch;

#[comp(dynec_as(crate), of = TestArch)]
struct Comp1(i32);

#[derive(Debug, PartialEq)]
#[comp(dynec_as(crate), of = TestArch)]
struct Comp2(i32);

#[test]
fn test_auto_init_fn() {
    let auto_fn = (|comp1: &Comp1| Comp2(comp1.0 + 5)) as fn(&_) -> _;
    let mut map = Map::default();
    map.insert_simple(Comp1(2));
    SimpleInitFn::<TestArch>::populate(&auto_fn, &mut map);
    assert_eq!(map.get_simple::<Comp2>(), Some(&Comp2(7)));
}
