use std::time::Duration;

use criterion::*;
use dynec::{system, test_util};
use rand::Rng;

#[dynec::comp(of = test_util::TestArch, required)]
struct PositionX(f64);
#[dynec::comp(of = test_util::TestArch, required)]
struct PositionY(f64);
#[dynec::comp(of = test_util::TestArch, required)]
struct PositionZ(f64);
#[dynec::comp(of = test_util::TestArch, required)]
struct VelocityX(f64);
#[dynec::comp(of = test_util::TestArch, required)]
struct VelocityY(f64);
#[dynec::comp(of = test_util::TestArch, required)]
struct VelocityZ(f64);

#[system]
fn system_individual_add_system_non_chunked(
    mut px: impl system::WriteSimple<test_util::TestArch, PositionX>,
    mut py: impl system::WriteSimple<test_util::TestArch, PositionY>,
    mut pz: impl system::WriteSimple<test_util::TestArch, PositionZ>,
    vx: impl system::ReadSimple<test_util::TestArch, VelocityX>,
    vy: impl system::ReadSimple<test_util::TestArch, VelocityY>,
    vz: impl system::ReadSimple<test_util::TestArch, VelocityZ>,
    entities: impl system::EntityIterator<test_util::TestArch>,
) {
    for (_, (px, py, pz, vx, vy, vz)) in entities.entities_with((
        px.access_mut(),
        py.access_mut(),
        pz.access_mut(),
        vx.access(),
        vy.access(),
        vz.access(),
    )) {
        px.0 += vx.0;
        py.0 += vy.0;
        pz.0 += vz.0;
    }
}

fn iter_entity_add_individual_non_chunked(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter entity (a += b)");
    group.measurement_time(Duration::from_secs(10));

    for log_entities in (4..=16).step_by(4) {
        let num_entities = 1 << log_entities;
        group.throughput(Throughput::Elements(num_entities));
        group.bench_with_input(
            BenchmarkId::new("individual/non-chunked", format!("{num_entities} entities")),
            &num_entities,
            |b, &num_entities| {
                let mut world =
                    dynec::system_test!(system_individual_add_system_non_chunked.build(););
                let mut rng = rand::thread_rng();
                for _ in 0..num_entities {
                    world.create(dynec::comps![test_util::TestArch =>
                        PositionX(rng.gen_range(-65536.0 ..= 65536.0)),
                        PositionY(rng.gen_range(-65536.0 ..= 65536.0)),
                        PositionZ(rng.gen_range(-65536.0 ..= 65536.0)),
                        VelocityX(rng.gen_range(-65536.0 ..= 65536.0)),
                        VelocityY(rng.gen_range(-65536.0 ..= 65536.0)),
                        VelocityZ(rng.gen_range(-65536.0 ..= 65536.0)),
                    ]);
                }
                b.iter(|| {
                    world.execute(&dynec::tracer::Noop);
                })
            },
        );
    }
}

#[dynec::comp(of = test_util::TestArch, required)]
struct PositionArray([f64; 3]);
#[dynec::comp(of = test_util::TestArch, required)]
struct VelocityArray([f64; 3]);

#[system]
fn system_array_add_system_non_chunked(
    mut p: impl system::WriteSimple<test_util::TestArch, PositionArray>,
    v: impl system::ReadSimple<test_util::TestArch, VelocityArray>,
    entities: impl system::EntityIterator<test_util::TestArch>,
) {
    for (_, (p, v)) in entities.entities_with((p.access_mut(), v.access())) {
        for i in 0..3 {
            p.0[i] += v.0[i];
        }
    }
}

fn iter_entity_add_array_non_chunked(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter entity (a += b)");
    group.measurement_time(Duration::from_secs(10));

    for log_entities in (4..=16).step_by(4) {
        let num_entities = 1 << log_entities;
        group.throughput(Throughput::Elements(num_entities));
        group.bench_with_input(
            BenchmarkId::new("array/non-chunked", format!("{num_entities} entities")),
            &num_entities,
            |b, &num_entities| {
                let mut world = dynec::system_test!(system_array_add_system_non_chunked.build(););
                let mut rng = rand::thread_rng();
                for _ in 0..num_entities {
                    world.create(dynec::comps![test_util::TestArch =>
                        PositionArray([
                            rng.gen_range(-65536.0 ..= 65536.0),
                            rng.gen_range(-65536.0 ..= 65536.0),
                            rng.gen_range(-65536.0 ..= 65536.0),
                        ]),
                        VelocityArray([
                            rng.gen_range(-65536.0 ..= 65536.0),
                            rng.gen_range(-65536.0 ..= 65536.0),
                            rng.gen_range(-65536.0 ..= 65536.0),
                        ]),
                    ]);
                }
                b.iter(|| {
                    world.execute(&dynec::tracer::Noop);
                })
            },
        );
    }
}

criterion_group!(individual_non_chunked, iter_entity_add_individual_non_chunked);
criterion_group!(array_non_chunked, iter_entity_add_array_non_chunked);
criterion_main!(individual_non_chunked, array_non_chunked);
