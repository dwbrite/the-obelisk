# Fold documentation

Fold is a Rust library for **incrementally-maintained, persistent dataflow**.
You describe *what* derived state you want — counts, multisets, aggregates,
inverted indexes — as a pipeline of operators, and Fold keeps all of it up to
date as records are inserted and removed. Nothing is ever recomputed from
scratch, and everything lives in an embedded, crash-safe store.

```rust
use fold::pipeline::{Filter, Map, terminal};
use fold::stream::Stream;

let mut st = Stream::new(
    "example.db",
    Filter::new(
        |s: &String| !s.is_empty(),
        (
            terminal::Count::new("total"),
            Map::new(|s: &String| s.len(), terminal::Bag::new("lengths")),
        ),
    ),
);

st.wtx(|tx| {
    tx.insert(&"hello".to_string());
    tx.insert(&"world".to_string());
});

st.rtx(|(count, lengths)| {
    assert_eq!(count.get(), 2);
    assert!(lengths.contains(&5));
});
```

## Reading order

1. [Concepts](concepts.md) — deltas, retraction, pipelines, sinks, and the
   mental model behind everything else.
2. [Getting started](getting-started.md) — build a small program end to end.
3. [Operators](operators.md) — reference for every pipeline operator.
4. [Sinks](sinks.md) — reference for the terminal sinks and their readers.
5. [Transactions & persistence](transactions.md) — atomicity, snapshots,
   crash safety, and reopening a store.
6. [Writing your own operators](extending.md) — the `Push` trait and node
   lifecycle, for when the built-ins aren't enough.

## The elevator pitch

Most systems answer "how many users signed up?" by scanning a table. Fold
answers it by *maintaining* the count: every insert bumps it, every delete
decrements it, and reading it is a single key lookup. The same idea scales to
per-key aggregates, distinct sets, and full-text-style inverted indexes — all
declared once, all updated together in one atomic transaction, all readable
from one consistent snapshot.
