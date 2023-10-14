use std::iter;
use std::time::Duration;

use criterion::measurement::WallTime;
use criterion::*;
use dynec::test_util::TestArch;
use dynec::{comp, system};
use rand::rngs::ThreadRng;
use rand::Rng;

#[dynec::comp(of = TestArch, required)]
struct PositionX(f64);
#[dynec::comp(of = TestArch, required)]
struct PositionY(f64);
#[dynec::comp(of = TestArch, required)]
struct PositionZ(f64);
#[dynec::comp(of = TestArch, required)]
struct VelocityX(f64);
#[dynec::comp(of = TestArch, required)]
struct VelocityY(f64);
#[dynec::comp(of = TestArch, required)]
struct VelocityZ(f64);

fn individual_comps(rng: &mut ThreadRng) -> comp::Map<TestArch> {
    dynec::comps![TestArch =>
        PositionX(rng.gen_range(-65536.0 ..= 65536.0)),
        PositionY(rng.gen_range(-65536.0 ..= 65536.0)),
        PositionZ(rng.gen_range(-65536.0 ..= 65536.0)),
        VelocityX(rng.gen_range(-65536.0 ..= 65536.0)),
        VelocityY(rng.gen_range(-65536.0 ..= 65536.0)),
        VelocityZ(rng.gen_range(-65536.0 ..= 65536.0)),
    ]
}

#[system]
fn system_individual_add_system_non_chunked(
    mut px: system::WriteSimple<TestArch, PositionX>,
    mut py: system::WriteSimple<TestArch, PositionY>,
    mut pz: system::WriteSimple<TestArch, PositionZ>,
    vx: system::ReadSimple<TestArch, VelocityX>,
    vy: system::ReadSimple<TestArch, VelocityY>,
    vz: system::ReadSimple<TestArch, VelocityZ>,
    entities: system::EntityIterator<TestArch>,
) {
    for (_, (px, py, pz, vx, vy, vz)) in
        entities.entities_with((&mut px, &mut py, &mut pz, &vx, &vy, &vz))
    {
        px.0 += vx.0;
        py.0 += vy.0;
        pz.0 += vz.0;
    }
}

#[system]
fn system_individual_add_system_chunked(
    mut px: system::WriteSimple<TestArch, PositionX>,
    mut py: system::WriteSimple<TestArch, PositionY>,
    mut pz: system::WriteSimple<TestArch, PositionZ>,
    vx: system::ReadSimple<TestArch, VelocityX>,
    vy: system::ReadSimple<TestArch, VelocityY>,
    vz: system::ReadSimple<TestArch, VelocityZ>,
    entities: system::EntityIterator<TestArch>,
) {
    for (_, (px, py, pz, vx, vy, vz)) in
        entities.chunks_with((&mut px, &mut py, &mut pz, &vx, &vy, &vz))
    {
        for (px, (py, (pz, (vx, (vy, vz))))) in
            iter::zip(px, iter::zip(py, iter::zip(pz, iter::zip(vx, iter::zip(vy, vz)))))
        {
            px.0 += vx.0;
            py.0 += vy.0;
            pz.0 += vz.0;
        }
    }
}

#[dynec::comp(of = TestArch, required)]
struct PositionArray([f64; 3]);
#[dynec::comp(of = TestArch, required)]
struct VelocityArray([f64; 3]);

fn array_comps(rng: &mut ThreadRng) -> comp::Map<TestArch> {
    dynec::comps![TestArch =>
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
    ]
}

#[system]
fn system_array_add_system_non_chunked(
    mut p: system::WriteSimple<TestArch, PositionArray>,
    v: system::ReadSimple<TestArch, VelocityArray>,
    entities: system::EntityIterator<TestArch>,
) {
    for (_, (p, v)) in entities.entities_with((&mut p, &v)) {
        for i in 0..3 {
            p.0[i] += v.0[i];
        }
    }
}

#[system]
fn system_array_add_system_chunked(
    mut p: system::WriteSimple<TestArch, PositionArray>,
    v: system::ReadSimple<TestArch, VelocityArray>,
    entities: system::EntityIterator<TestArch>,
) {
    for (_, (p, v)) in entities.chunks_with((&mut p, &v)) {
        for (p, v) in iter::zip(p, v) {
            for i in 0..3 {
                p.0[i] += v.0[i];
            }
        }
    }
}

fn bench_iter_entity_add<SystemT, DeleteEntityIter>(
    group: &mut BenchmarkGroup<'_, measurement::WallTime>,
    subgroup: &str,
    function_name: &str,
    build_system: impl Fn() -> SystemT,
    make_comps: impl Fn(&mut ThreadRng) -> comp::Map<TestArch>,
    entities_to_delete: impl Fn(u64) -> DeleteEntityIter,
) where
    SystemT: system::Sendable,
    DeleteEntityIter: Iterator<Item = u64>,
{
    group.measurement_time(Duration::from_secs(10));

    let num_entities = 65536;
    group.throughput(Throughput::Elements(num_entities));
    group.bench_with_input(
        BenchmarkId::new(function_name, subgroup),
        &num_entities,
        |b, &num_entities| {
            let mut world = dynec::system_test!(build_system(););
            let mut rng = rand::thread_rng();
            let mut entities: Vec<_> =
                (0..num_entities).map(|_| world.create(make_comps(&mut rng))).map(Some).collect();
            for pos in entities_to_delete(num_entities) {
                let entity = entities
                    .get_mut(pos as usize)
                    .expect("entities_to_delete yielded overflowing values");
                let entity = entity.take().expect("entities_to_delete yielded repeated values");
                world.delete(entity);
            }
            b.iter(|| world.execute(&dynec::tracer::Noop))
        },
    );
}

fn iter_entity_add_with_deletion<DeleteEntityIter: Iterator<Item = u64>>(
    group: &mut BenchmarkGroup<WallTime>,
    name: &str,
    deletion: impl Fn(u64) -> DeleteEntityIter + Copy,
) {
    bench_iter_entity_add(
        group,
        name,
        "ent idv",
        || system_individual_add_system_non_chunked.build(),
        individual_comps,
        deletion,
    );
    bench_iter_entity_add(
        group,
        name,
        "chunk idv",
        || system_individual_add_system_chunked.build(),
        individual_comps,
        deletion,
    );
    bench_iter_entity_add(
        group,
        name,
        "ent arr",
        || system_array_add_system_non_chunked.build(),
        array_comps,
        deletion,
    );
    bench_iter_entity_add(
        group,
        name,
        "chunk arr",
        || system_array_add_system_chunked.build(),
        array_comps,
        deletion,
    );
}

fn iter_entity_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter entity (p += v)");

    iter_entity_add_with_deletion(&mut group, "full", |_| iter::empty());

    iter_entity_add_with_deletion(&mut group, "holes", |total| {
        iter::repeat([(1_u64, 2), (4, 8), (16, 12)].into_iter())
            .flatten()
            .scan(0, |state, (keep, delete)| {
                *state += keep;
                let start = *state;
                *state += delete;
                let end = *state;
                Some(start..end)
            })
            .flatten()
            .take_while(move |&index| index < total)
    });
}

criterion_group!(benches, iter_entity_add);
criterion_main!(benches);
