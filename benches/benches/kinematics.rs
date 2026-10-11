// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared forward kinematics over every vertex of the 1M-triangle benchmark model.
//! A baseline for the 10% regression flag (verification plan section 4.1).

#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use openchuck_testkit::parts::benchmark_model_1m;
use openchuck_types::kinematics::machine_xyz;
use openchuck_types::machine::MachineProfile;
use openchuck_types::units::Degrees;

fn kinematics(c: &mut Criterion) {
    let mesh = benchmark_model_1m();
    let profile = MachineProfile::example_x_rotary();
    c.bench_function("machine_xyz over 1M-triangle model vertices", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for (i, v) in mesh.vertices.iter().enumerate() {
                let a = Degrees((i % 720) as f64 * 0.5);
                sum += machine_xyz(&profile, *v, a).z;
            }
            std::hint::black_box(sum)
        });
    });
}

criterion_group!(benches, kinematics);
criterion_main!(benches);
