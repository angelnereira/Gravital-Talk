//! Benchmark: PCM (and Opus) encode of a 10 ms frame at 48 kHz mono.
//! Target: ≤ 800 µs per encode call on release build.

use criterion::{criterion_group, criterion_main, Criterion};
use gravital_sound::{build_codec_pair, CodecId};

fn bench_pcm_encode(c: &mut Criterion) {
    let sample_rate = 48_000u32;
    let channels = 1u8;
    let frame_ms = 10u8;
    let (mut enc, _dec) =
        build_codec_pair(CodecId::Pcm, sample_rate, channels, frame_ms).expect("build pcm pair");
    let samples: Vec<i16> = (0..480).map(|i| (i as i16 * 100) % i16::MAX).collect();
    let mut out = vec![0u8; 4096];

    c.bench_function("pcm_encode_480samples", |b| {
        b.iter(|| {
            let _ = enc.encode(&samples, &mut out).unwrap();
        });
    });
}

fn bench_pcm_decode(c: &mut Criterion) {
    let sample_rate = 48_000u32;
    let channels = 1u8;
    let frame_ms = 10u8;
    let (mut enc, mut dec) =
        build_codec_pair(CodecId::Pcm, sample_rate, channels, frame_ms).expect("build pcm pair");
    let samples: Vec<i16> = (0..480).map(|i| (i as i16 * 100) % i16::MAX).collect();
    let mut encoded = vec![0u8; 4096];
    let n = enc.encode(&samples, &mut encoded).unwrap();
    let encoded = &encoded[..n];
    let mut pcm = vec![0i16; 1024];

    c.bench_function("pcm_decode_480samples", |b| {
        b.iter(|| {
            let _ = dec.decode(encoded, &mut pcm).unwrap();
        });
    });
}

#[cfg(feature = "opus")]
fn bench_opus_encode(c: &mut Criterion) {
    let sample_rate = 48_000u32;
    let channels = 1u8;
    let frame_ms = 10u8;
    let (mut enc, _dec) =
        build_codec_pair(CodecId::Opus, sample_rate, channels, frame_ms).expect("build opus pair");
    let samples: Vec<i16> = (0..480)
        .map(|i| {
            (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 48000.0)
                .sin()
                .mul_add(16000.0, 0.0) as i16
        })
        .collect();
    let mut out = vec![0u8; 4096];

    c.bench_function("opus_encode_480samples", |b| {
        b.iter(|| {
            let _ = enc.encode(&samples, &mut out).unwrap();
        });
    });
}

#[cfg(feature = "opus")]
fn bench_opus_decode(c: &mut Criterion) {
    let sample_rate = 48_000u32;
    let channels = 1u8;
    let frame_ms = 10u8;
    let (mut enc, mut dec) =
        build_codec_pair(CodecId::Opus, sample_rate, channels, frame_ms).expect("build opus pair");
    let samples: Vec<i16> = (0..480)
        .map(|i| {
            (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 48000.0)
                .sin()
                .mul_add(16000.0, 0.0) as i16
        })
        .collect();
    let mut encoded = vec![0u8; 4096];
    let n = enc.encode(&samples, &mut encoded).unwrap();
    let encoded = encoded[..n].to_vec();
    let mut pcm = vec![0i16; 1024];

    c.bench_function("opus_decode_480samples", |b| {
        b.iter(|| {
            let _ = dec.decode(&encoded, &mut pcm).unwrap();
        });
    });
}

#[cfg(not(feature = "opus"))]
criterion_group!(benches, bench_pcm_encode, bench_pcm_decode);
#[cfg(feature = "opus")]
criterion_group!(
    benches,
    bench_pcm_encode,
    bench_pcm_decode,
    bench_opus_encode,
    bench_opus_decode
);
criterion_main!(benches);
