use std::iter;
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
fn iter_entity_add_individual_non_chunked(group: &mut BenchmarkGroup<'_, measurement::WallTime>) {
    group.measurement_time(Duration::from_secs(10));

    for log_entities in [12, 16] {
        let num_entities = 1 << log_entities;
        group.throughput(Throughput::Elements(num_entities));
        group.bench_with_input(
            BenchmarkId::new("non-chunked (x, y, z)", format!("{num_entities} entities")),
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

#[system]
fn system_individual_add_system_chunked(
    mut px: impl system::WriteSimple<test_util::TestArch, PositionX>,
    mut py: impl system::WriteSimple<test_util::TestArch, PositionY>,
    mut pz: impl system::WriteSimple<test_util::TestArch, PositionZ>,
    vx: impl system::ReadSimple<test_util::TestArch, VelocityX>,
    vy: impl system::ReadSimple<test_util::TestArch, VelocityY>,
    vz: impl system::ReadSimple<test_util::TestArch, VelocityZ>,
    entities: impl system::EntityIterator<test_util::TestArch>,
) {
    for (_, (px, py, pz, vx, vy, vz)) in entities.chunks_with((
        px.access_chunk_mut(),
        py.access_chunk_mut(),
        pz.access_chunk_mut(),
        vx.access_chunk(),
        vy.access_chunk(),
        vz.access_chunk(),
    )) {
        for (px, (py, (pz, (vx, (vy, vz))))) in
            iter::zip(px, iter::zip(py, iter::zip(pz, iter::zip(vx, iter::zip(vy, vz)))))
        {
            px.0 += vx.0;
            py.0 += vy.0;
            pz.0 += vz.0;
        }
    }
}
fn iter_entity_add_individual_chunked(group: &mut BenchmarkGroup<'_, measurement::WallTime>) {
    group.measurement_time(Duration::from_secs(10));

    for log_entities in [12, 16] {
        let num_entities = 1 << log_entities;
        group.throughput(Throughput::Elements(num_entities));
        group.bench_with_input(
            BenchmarkId::new("chunked (x,y,z)", format!("{num_entities} entities")),
            &num_entities,
            |b, &num_entities| {
                let mut world = dynec::system_test!(system_individual_add_system_chunked.build(););
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
fn iter_entity_add_array_non_chunked(group: &mut BenchmarkGroup<'_, measurement::WallTime>) {
    group.measurement_time(Duration::from_secs(10));

    for log_entities in [12, 16] {
        let num_entities = 1 << log_entities;
        group.throughput(Throughput::Elements(num_entities));
        group.bench_with_input(
            BenchmarkId::new("non-chunked [f64; 3]", format!("{num_entities} entities")),
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

#[system]
fn system_array_add_system_chunked(
    mut p: impl system::WriteSimple<test_util::TestArch, PositionArray>,
    v: impl system::ReadSimple<test_util::TestArch, VelocityArray>,
    entities: impl system::EntityIterator<test_util::TestArch>,
) {
    for (_, (p, v)) in entities.chunks_with((p.access_chunk_mut(), v.access_chunk())) {
        for (p, v) in iter::zip(p, v) {
            for i in 0..3 {
                p.0[i] += v.0[i];
            }
        }
    }
}
fn iter_entity_add_array_chunked(group: &mut BenchmarkGroup<'_, measurement::WallTime>) {
    group.measurement_time(Duration::from_secs(10));

    for log_entities in [12, 16] {
        let num_entities = 1 << log_entities;
        group.throughput(Throughput::Elements(num_entities));
        group.bench_with_input(
            BenchmarkId::new("chunked [f64; 3]", format!("{num_entities} entities")),
            &num_entities,
            |b, &num_entities| {
                let mut world = dynec::system_test!(system_array_add_system_chunked.build(););
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

fn iter_entity_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter entity (p += v)");
    iter_entity_add_individual_non_chunked(&mut group);
    iter_entity_add_individual_chunked(&mut group);
    iter_entity_add_array_non_chunked(&mut group);
    iter_entity_add_array_chunked(&mut group);
}

criterion_group!(benches, iter_entity_add);
criterion_main!(benches);
