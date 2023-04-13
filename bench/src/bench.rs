extern crate criterion;
extern crate rs_libinjection;
extern crate testutil;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rs_libinjection::is_sqli;
use testutil::get_sqli_tests;

pub fn sqli_benchmark(c: &mut Criterion) {
    let mut i = 0;
    for test in get_sqli_tests() {
        c.bench_function(&test.name, |b| b.iter(|| is_sqli(black_box(&test.input))));
        i += 1;
    }
}

criterion_group!(benches, sqli_benchmark);
criterion_main!(benches);
