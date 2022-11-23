use criterion::*;
use dynec::test_util;

fn create_entity(c: &mut Criterion) {
    let mut group = c.benchmark_group("create entity");

    macro_rules! create_entity_batch {
        ($num_comps:literal; $($comps:expr),* $(,)?) => {
            for log_entities in (0..=8).step_by(4) {
                let entities = 1 << log_entities;
                group.throughput(Throughput::Elements(entities));
                group.bench_with_input(BenchmarkId::new(format!("{} components", $num_comps), format!("{entities} entities")), &entities, |b, &entities| {
                    b.iter_batched(
                        || dynec::system_test!(test_util::use_comp_n.build();),
                        |mut world| {
                            for _ in 0..entities {
                                world.create(dynec::comps![test_util::TestArch =>
                                    $($comps),*
                                ]);
                            }
                        },
                        BatchSize::SmallInput,
                    );
                });
            }
        }
    }

    create_entity_batch!(0; );
    create_entity_batch!(1; test_util::CompN::<1>(1));
    create_entity_batch!(2; test_util::CompN::<1>(1), test_util::CompN::<2>(2));
    create_entity_batch!(4; test_util::CompN::<1>(1), test_util::CompN::<2>(2), test_util::CompN::<3>(3), test_util::CompN::<4>(4));
    create_entity_batch!(8; test_util::CompN::<1>(1), test_util::CompN::<2>(2), test_util::CompN::<3>(3), test_util::CompN::<4>(4), test_util::CompN::<5>(5), test_util::CompN::<6>(6), test_util::CompN::<7>(7), test_util::CompN::<8>(8));
    create_entity_batch!(16; test_util::CompN::<1>(1), test_util::CompN::<2>(2), test_util::CompN::<3>(3), test_util::CompN::<4>(4), test_util::CompN::<5>(5), test_util::CompN::<6>(6), test_util::CompN::<7>(7), test_util::CompN::<8>(8), test_util::CompN::<9>(9), test_util::CompN::<10>(10), test_util::CompN::<11>(11), test_util::CompN::<12>(12), test_util::CompN::<13>(13), test_util::CompN::<14>(14), test_util::CompN::<15>(15), test_util::CompN::<16>(16));
}

criterion_group!(benches, create_entity);
criterion_main!(benches);
