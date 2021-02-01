use criterion::{criterion_group, criterion_main, Criterion};

use tunneler::{Message, MessageHeader, MessageType};

pub fn criterion_benchmark(c: &mut Criterion) {
    let data = [0; 13];
    c.bench_function("Deserialize-Message-Header", |b| {
        b.iter(|| MessageHeader::deserialize(data))
    });
    let header = MessageHeader::new(132, MessageType::Data, 200);
    c.bench_function("Serialize-Message-Header", |b| {
        b.iter(|| header.serialize())
    });

    let msg = Message::new(header, vec![0; 4092]);
    c.bench_function("Serialize-Message", |b| b.iter(|| msg.serialize()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
