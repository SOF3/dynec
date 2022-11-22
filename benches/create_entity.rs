use criterion::*;
use dynec::test_util;

fn create_entity_empty(c: &mut Criterion) {
    c.bench_function("create entity", |b| {
        b.iter(|| {
            let mut world = dynec::system_test!(test_util::use_comp_n.build(););
            world.create(dynec::comps![test_util::TestArch =>]);
        })
    });
}

criterion_group!(benches, create_entity_empty);
criterion_main!(benches);
