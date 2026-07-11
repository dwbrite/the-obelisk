# Operators

Operators are the interior nodes of a pipeline: they transform or route
deltas on their way to [sinks](sinks.md). All of them live in
`fold::pipeline`. Every operator's constructor takes its downstream node
last, so pipelines are written inside-out (leaves innermost).

Operators come in three flavors:

| Flavor | Operators | State |
|---|---|---|
| Stateless | `Map`, `Filter`, `FilterMap`, `FlatMap` | none — deltas forwarded immediately |
| Stateful | `Distinct`, `Aggregate` | persisted in a named keyspace; flushed at commit |
| Keying | `KeyBy`, `Unkey` | none — convert between plain and `Keyed` streams |

Plus **tuples**, which fan a stream out to several branches.

## Stateless operators

These forward transformed deltas immediately and hold no state. Their
functions **must be deterministic**: a retraction re-runs the function on the
original datum, and the output must match what was originally inserted or
downstream state won't cancel.

### `Map`

Applies a function to each datum.

```rust
Map::new(|s: &String| s.len(), /* next: Push<usize> */)
```

### `Filter`

Forwards only data passing a predicate. Because the predicate is
deterministic, an insert and its retraction either both pass or both drop.

```rust
Filter::new(|s: &String| !s.is_empty(), /* next: Push<String> */)
```

### `FilterMap`

`Map` and `Filter` in one step: forwards `Some` results, drops `None`.

```rust
FilterMap::new(|s: &String| s.parse::<u64>().ok(), /* next: Push<u64> */)
```

### `FlatMap`

Expands each datum into zero or more outputs, each forwarded with the input's
delta. The canonical use is tokenizing documents for an inverted index:

```rust
FlatMap::new(
    |d: &Document| {
        let id = d.id;
        d.body
            .split_ascii_whitespace()
            .map(move |w| Keyed::new(id, w.to_ascii_lowercase()))
            .collect::<Vec<_>>()
    },
    terminal::InvertedIndex::new("words"),
)
```

## Stateful operators

Stateful operators persist per-element state in their own keyspace (the
`name` argument; unique pipeline-wide). Within a transaction they only buffer
in memory; at commit they reconcile the buffer against the store and emit the
resulting downstream deltas. Hot elements within one transaction therefore
collapse to at most one store update.

### `Distinct`

Collapses multiplicity to set semantics. It tracks each element's total
multiplicity and emits downstream only on the edges: `+1` when an element's
count crosses 0 → positive, `-1` when it crosses back to 0. Downstream sees
each element at most once, no matter how many copies exist upstream.

```rust
Distinct::new("seen", terminal::Count::new("unique_count"))
```

Requires `D: Serialize` (elements are keyed by their postcard encoding).

### `Aggregate`

Per-key incremental aggregation over `Keyed<K, V>` streams — see below for
how streams become keyed. Persists `(record_count, accumulator)` per key.

```rust
// sum of u64 values, grouped by residue mod 3
KeyBy::new(
    |v: &u64| v % 3,
    Aggregate::new(
        "sums",
        |acc: &mut i64, v: &u64, delta| *acc += *v as i64 * delta as i64,
        Unkey::new(terminal::Bag::new("sum_bag")),
    ),
)
```

The step function `Fn(&mut A, &V, isize)` receives the **delta** and must be
invertible with respect to negative deltas: applying `(v, +1)` then `(v, -1)`
must leave the accumulator unchanged. Sums, counts, and products satisfy
this; `min`/`max` do not (you'd need to keep enough state to recompute on
retraction, e.g. a bag of values).

The accumulator type `A` needs `Clone + Default + Serialize +
DeserializeOwned`; a key's accumulator starts at `A::default()`.

**Downstream sees a changelog**, not raw values: when a key's aggregate
changes in a transaction, `Aggregate` emits `Keyed { key, old_acc }` with
delta `-1` followed by `Keyed { key, new_acc }` with delta `+1`. A sink fed
(via `Unkey`) from an aggregate therefore always holds exactly the current
aggregate per key — the retraction removes the stale value. When a key's
record count drops to 0, only the retraction is emitted and the key's state
is deleted.

## Keyed operators

Grouping is carried in the type system by
[`Keyed<K, V>`](../src/pipeline/mod.rs) — a value paired with its grouping
key.

### `KeyBy`

Turns `Push<V>` input into `Keyed<K, V>` output by computing a key from each
datum. The key function must be deterministic so retractions land on the same
key. This is the entry point to `Aggregate`.

```rust
KeyBy::new(|d: &Document| d.label, /* next: Push<Keyed<u32, Document>> */)
```

### `Unkey`

The inverse: discards the key and forwards the bare value. Typically placed
after `Aggregate` when downstream no longer cares about grouping (e.g. to
land accumulators in a `Bag`).

```rust
Unkey::new(terminal::Bag::new("accs"))
```

## Fan-out: tuples

Any tuple of up to 16 `Push` nodes is itself a `Push` node that broadcasts
each delta to every element, in order. Use a tuple anywhere a downstream node
is expected to split the pipeline into parallel branches:

```rust
Filter::new(
    |v: &String| v.len() > 1,
    (
        Map::new(|v: &String| v.len(), terminal::Count::new("len_count")),
        terminal::Bag::new("raw"),
        Map::new(|v: &String| v.to_uppercase(), terminal::Bag::new("uppers")),
    ),
)
```

A tuple's reader is the tuple of its elements' readers, so the `rtx` closure
above receives `(count_reader, bag_reader, bag_reader)`. Branches can nest
arbitrarily; the reader structure always mirrors the sink structure.
