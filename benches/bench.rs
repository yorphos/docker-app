use criterion::{black_box, criterion_group, criterion_main, Criterion};
use docker_app::{start_serving, Broker};
use tokio::runtime::Runtime;

fn criterion_benchmark(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let broker = Box::leak(Box::new(Broker {
        id: "docker".to_string(),
        host: "192.168.0.122".to_string(),
        port: 1883,
    }));

    c.bench_function("app", |b| {
        b.to_async(&runtime)
            .iter(|| async { black_box(start_serving(broker).await) })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
