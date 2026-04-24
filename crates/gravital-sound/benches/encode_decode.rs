//! Benchmark Criterion del encode/decode de paquetes.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use gravital_sound::{PacketBuilder, PacketHeader, PacketView};

fn bench_encode(c: &mut Criterion) {
    let header = PacketHeader::new(0x10, 0xDEAD_BEEF, 0, 0);
    let payload = vec![0u8; 960];
    let mut out = vec![0u8; 1200];

    let mut group = c.benchmark_group("packet");
    group.throughput(Throughput::Bytes(960));
    group.bench_function("encode_960B", |b| {
        b.iter(|| {
            let n = PacketBuilder::new(header, black_box(&payload))
                .encode(&mut out)
                .unwrap();
            black_box(n);
        })
    });

    // Pre-encode once for decode.
    let n = PacketBuilder::new(header, &payload)
        .encode(&mut out)
        .unwrap();
    let encoded = out[..n].to_vec();
    group.bench_function("decode_960B", |b| {
        b.iter(|| {
            let v = PacketView::decode(black_box(&encoded)).unwrap();
            black_box(v);
        })
    });
    group.finish();
}

criterion_group!(benches, bench_encode);
criterion_main!(benches);
