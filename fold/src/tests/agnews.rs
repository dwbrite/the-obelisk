use crate::{pipeline::*, stream::*};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
struct Document {
    id: usize,
    title: String,
    body: String,
    label: u32,
}

fn normalize(tok: &str) -> Option<String> {
    let t: String = tok.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    (!t.is_empty()).then(|| t.to_ascii_lowercase())
}

fn parse_samples(data: &str) -> Vec<Document> {
    let mut lines = data.lines();
    let mut samples = Vec::new();
    let mut i = 0;
    while let Some(title) = lines.next() {
        let body = lines.next().expect("missing description").to_string();
        let label = lines
            .next()
            .expect("missing label")
            .parse()
            .expect("invalid label");
        samples.push(Document {
            id: i,
            title: title.to_string(),
            body,
            label,
        });
        i += 1;
    }
    samples
}

#[test]
fn agnews_inverted_index() {
    let data = include_str!("testdata/agnews.txt");
    let samples = parse_samples(data);
    let iters = samples.len();
    let total_bytes: usize = samples.iter().map(|d| d.body.len()).sum();

    let open_start = Instant::now();
    let mut st = Stream::new(
        "agnews.db",
        FlatMap::new(
            |d: &Document| {
                let title = d.title.clone();
                d.body
                    .split_ascii_whitespace()
                    .filter_map(normalize)
                    .map(move |tok| Keyed::new(title.clone(), tok))
                    .collect::<Vec<_>>()
            },
            terminal::InvertedIndex::new("agnews_ii"),
        ),
    );
    let open_dur = open_start.elapsed();

    let start = Instant::now();
    for chunk in samples.chunks(5_000) {
        st.wtx(|tx| {
            for doc in chunk {
                tx.insert(doc);
            }
        });
    }
    let ingest = start.elapsed();

    let queries = [
        "the",
        "government",
        "microsoft",
        "oil",
        "zzzzzzzzzzzzzz",
        "georgia",
        "china",
    ];
    st.rtx(|ii| {
        for q in queries {
            let start = Instant::now();
            let hits = ii.search(&q.to_string());
            println!("search {q:?}: {} hits in {:?}", hits.len(), start.elapsed());
        }
    });

    let start = Instant::now();
    st.wtx(|tx| {
        for doc in &samples {
            tx.remove(doc);
        }
    });
    let del = start.elapsed();

    st.rtx(|ii| {
        for q in queries {
            assert!(ii.search(&q.to_string()).is_empty());
        }
    });

    println!("open/recover: {open_dur:?}");
    println!(
        "ingest: {iters} docs ({:.1} MB) in {ingest:?} (avg {:?}/doc, {:.1} MB/s)",
        total_bytes as f64 / 1e6,
        ingest.div_f64(iters as f64),
        total_bytes as f64 / 1e6 / ingest.as_secs_f64()
    );
    println!(
        "delete: {iters} docs in {del:?} (avg {:?}/doc)",
        del.div_f64(iters as f64)
    );
}
