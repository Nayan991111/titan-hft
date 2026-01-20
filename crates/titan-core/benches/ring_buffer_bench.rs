use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use titan_core::buffer::RingBuffer;
use titan_common::MarketTick;
use std::thread;
use std::sync::Arc;
use std::time::Duration;

fn bench_ring_buffer_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer");
    
    // Measure elements per second
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10)); 

    group.bench_function("spsc_throughput", |b| {
        b.iter(|| {
            let ring = Arc::new(RingBuffer::new());
            let producer_ring = ring.clone();
            let consumer_ring = ring.clone();

            let producer = thread::spawn(move || {
                for _ in 0..10_000 {
                    let tick = MarketTick::new("AAPL", 150.0, 100, 1);
                    while !producer_ring.write(black_box(tick)) {
                        std::hint::spin_loop();
                    }
                }
            });

            let consumer = thread::spawn(move || {
                let mut count = 0;
                while count < 10_000 {
                    if let Some(tick) = consumer_ring.read() {
                        black_box(tick);
                        count += 1;
                    } else {
                        std::hint::spin_loop();
                    }
                }
            });

            producer.join().unwrap();
            consumer.join().unwrap();
        })
    });
    
    group.finish();
}

criterion_group!(benches, bench_ring_buffer_throughput);
criterion_main!(benches);