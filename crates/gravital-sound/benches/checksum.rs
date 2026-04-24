//! Benchmark del CRC-16/CCITT-FALSE.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use gravital_sound::checksum::crc16_ccitt_false;

fn bench_crc(c: &mut Criterion) {
    let mut group = c.benchmark_group("crc16");
    for &size in &[64usize, 512, 1024, 2048] {
        let buf = vec![0xA5u8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(format!("{}B", size), |b| {
            b.iter(|| {
                let crc = crc16_ccitt_false(black_box(&buf));
                black_box(crc);
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_crc);
criterion_main!(benches);
