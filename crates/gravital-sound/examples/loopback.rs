//! Benchmark loopback: mide p50/p99/p99.9 de latencia encode → decode.
//!
//! Ejecuta 1,000,000 iteraciones y reporta histograma con `hdrhistogram`.
//!
//! Uso:
//!   cargo run --release --example loopback

use std::time::Instant;

use gravital_sound::{PacketBuilder, PacketHeader, PacketView};
use hdrhistogram::Histogram;

fn main() {
    let iters = 1_000_000u64;
    let header = PacketHeader::new(0x10, 0xDEAD_BEEF, 0, 0);
    let payload = vec![0u8; 960];
    let mut out = vec![0u8; 1200];

    let mut hist: Histogram<u64> = Histogram::new(3).expect("histogram");

    for _ in 0..10_000 {
        // warmup
        let n = PacketBuilder::new(header, &payload)
            .encode(&mut out)
            .unwrap();
        let _ = PacketView::decode(&out[..n]).unwrap();
    }

    let start = Instant::now();
    for _ in 0..iters {
        let t0 = Instant::now();
        let n = PacketBuilder::new(header, &payload)
            .encode(&mut out)
            .unwrap();
        let _v = PacketView::decode(&out[..n]).unwrap();
        let ns = t0.elapsed().as_nanos() as u64;
        hist.record(ns).ok();
    }
    let elapsed = start.elapsed();

    println!("encode+decode loopback ({iters} iters, 960B payload)");
    println!("  total elapsed : {:?}", elapsed);
    println!(
        "  throughput    : {:.2} M ops/s",
        iters as f64 / elapsed.as_secs_f64() / 1_000_000.0
    );
    println!("  min           : {} ns", hist.min());
    println!("  p50           : {} ns", hist.value_at_quantile(0.50));
    println!("  p95           : {} ns", hist.value_at_quantile(0.95));
    println!("  p99           : {} ns", hist.value_at_quantile(0.99));
    println!("  p99.9         : {} ns", hist.value_at_quantile(0.999));
    println!("  max           : {} ns", hist.max());

    // Quality gate: p50 debe estar bajo 500 ns en hardware moderno.
    let p50 = hist.value_at_quantile(0.50);
    if p50 > 1_000 {
        eprintln!("WARN: p50 {p50} ns exceeds target 1000 ns — check release mode");
    }
}
