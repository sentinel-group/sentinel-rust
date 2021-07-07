#[macro_use]
extern crate criterion;
use criterion::Criterion;

mod fib {
    use super::*;
    fn fibonacci(n: u64) -> u64 {
        match n {
            0 => 1,
            1 => 1,
            n => fibonacci(n - 1) + fibonacci(n - 2),
        }
    }

    pub fn bench_fib(c: &mut Criterion) {
        c.bench_function("fib 20", |b| b.iter(|| fibonacci(20)));
    }
}

criterion_group!(benches, fib::bench_fib);
criterion_main!(benches);
