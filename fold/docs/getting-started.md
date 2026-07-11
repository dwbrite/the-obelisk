# Getting started

This walkthrough builds a small document-tracking program: ingest documents,
maintain a live count, a word index, and a per-label sum — then delete some
documents and watch everything stay consistent.

## Setup

```toml
[dependencies]
fold = "0.0.1"
serde = { version = "1", features = ["derive"] }
```

Your data types need `Clone` everywhere, plus `Serialize` (and
`DeserializeOwned` to read back out of sinks) — Fold stores everything via
[postcard](https://docs.rs/postcard).

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct Document {
    id: usize,
    body: String,
    label: u32,
}
```

## 1. Declare the pipeline

Decide what derived state you want, then write it as a tree. Here: a total
count, an inverted index from words to document ids, and a per-label count of
documents.

```rust
use fold::pipeline::{Aggregate, FlatMap, KeyBy, Unkey, terminal};
use fold::stream::Stream;

let mut st = Stream::new(
    "docs.db",
    (
        // Branch 1: how many documents are live?
        terminal::Count::new("total"),
        // Branch 2: word → document ids
        FlatMap::new(
            |d: &Document| {
                let id = d.id;
                d.body
                    .split_ascii_whitespace()
                    .map(move |w| (id, w.to_ascii_lowercase()))
                    .collect::<Vec<_>>()
            },
            terminal::InvertedIndex::new("words"),
        ),
        // Branch 3: label → number of documents with that label
        KeyBy::new(
            |d: &Document| d.label,
            Aggregate::new(
                "by_label",
                |acc: &mut i64, _d: &Document, delta| *acc += delta as i64,
                Unkey::new(terminal::Bag::new("label_counts")),
            ),
        ),
    ),
);
```

Notes on what just happened:

- The outermost node is a **tuple**, so every document fans out to all three
  branches.
- `FlatMap` expands one document into many `(id, word)` pairs; the
  `InvertedIndex` sink stores them as postings.
- `KeyBy` attaches `label` as the grouping key; `Aggregate` maintains one
  accumulator per label. Its step function receives the **delta** and must
  handle retraction — here `+= delta` is naturally invertible.
- Every named node (`"total"`, `"words"`, `"by_label"`, `"label_counts"`)
  claims a keyspace in `docs.db`. Names must be unique; `Stream::new` panics
  otherwise.

## 2. Write

All deltas pushed inside one `wtx` commit atomically:

```rust
let docs = vec![
    Document { id: 0, body: "the quick brown fox".into(), label: 1 },
    Document { id: 1, body: "the lazy dog".into(), label: 2 },
];

st.wtx(|tx| {
    for d in &docs {
        tx.insert(d);
    }
});
```

Batching matters for throughput: stateful nodes buffer within a transaction
and flush once at commit, so prefer one `wtx` around a chunk of records over
one `wtx` per record.

## 3. Read

The reader passed to `rtx` mirrors the pipeline's sink structure — a tuple of
three branches yields a tuple of three readers — and all of them see one
consistent snapshot:

```rust
st.rtx(|(count, words, labels)| {
    assert_eq!(count.get(), 2);

    // every doc id containing "the"
    let hits: Vec<usize> = words.search(&"the".to_string());
    assert_eq!(hits.len(), 2);

    // (label, doc-count) pairs — Keyed<u32, i64> unkeyed into a Bag
    for (per_label, _mult) in labels.iter() {
        let per_label: i64 = per_label;
        // …
    }
});
```

## 4. Remove — and watch retraction work

Removing a document pushes it back through the pipeline with delta `-1`. The
count decrements, its postings vanish from the index, and its label's
aggregate steps down — no rebuild, no tombstone scan:

```rust
st.wtx(|tx| tx.remove(&docs[0]));

st.rtx(|(count, words, _)| {
    assert_eq!(count.get(), 1);
    assert!(words.search(&"fox".to_string()).is_empty());
});
```

One rule to respect: `remove` re-runs your operator functions on the original
datum, so you must pass a value equal to what you inserted, and your `Map`/
`FlatMap`/`KeyBy` functions must be deterministic.

## 5. Restart

Everything is persistent. Drop the `Stream`, reopen the same path with the
same pipeline, and all sinks resume from the last committed transaction:

```rust
drop(st);
let st = Stream::new("docs.db", /* same pipeline as above */);
st.rtx(|(count, _, _)| assert_eq!(count.get(), 1));
```

Commits are durable against process crashes as soon as `wtx` returns. Call
`st.checkpoint()` to fsync when you also want protection against OS or power
failure.

## Where to next

- [Operators](operators.md) — everything you can put between the stream and
  a sink.
- [Sinks](sinks.md) — what you can materialize and how to read it.
- [Writing your own operators](extending.md) — when the built-ins don't cover
  your shape of state.
