# Sinks

Sinks are the leaves of a pipeline: nodes that persist materialized state
instead of forwarding it. They live in `fold::pipeline::terminal`.

Common rules:

- Every sink takes a `name` at construction. The name identifies its
  keyspace in the store (partition `sink_{name}`) and must be **unique among
  all named nodes in the pipeline** — `Stream::new` panics on a duplicate.
  Stable names are also what lets a reopened store resume: same path + same
  names = same state.
- Every sink honors retraction: pushing a datum with delta `-n` undoes `n`
  prior insertions.
- Sinks buffer deltas in memory during a transaction and reconcile with the
  store once at commit, so repeated pushes of a hot element in one batch are
  cheap.
- Elements are serialized with [postcard](https://docs.rs/postcard), so data
  types need `Serialize` (and `DeserializeOwned` to iterate back out).

You read a sink through its **reader**, obtained inside `Stream::rtx`. The
reader is pinned to one snapshot for the closure's duration, and a pipeline's
reader structure mirrors its sink structure (operators are transparent;
tuples yield tuples).

## `Count`

A persistent running sum of all deltas — i.e. the number of live records.
Accepts any data type and ignores the data itself.

```rust
terminal::Count::new("total")
```

Reader (`CountReader`):

| Method | Returns |
|---|---|
| `get()` | `i64` — current count; `0` if nothing was ever inserted |

## `Bag`

A persistent **counted multiset**: each distinct element maps to its
multiplicity (the running sum of that element's deltas). Elements whose
multiplicity reaches 0 are physically removed.

```rust
terminal::Bag::<String>::new("raw")
```

Reader (`BagReader<D>`):

| Method | Returns |
|---|---|
| `iter()` | iterator of `(D, i64)` — every element with its (always positive) multiplicity, ordered by the element's postcard encoding |
| `contains(&d)` | `bool` — whether `d` has multiplicity > 0 |

A `Bag` downstream of `Distinct` behaves as a set (all multiplicities 1);
a `Bag` downstream of `Unkey∘Aggregate` holds the current accumulator per
key, because the aggregate's changelog retracts stale values.

## `InvertedIndex`

A persistent inverted index over `Keyed<K,V>` pairs: look up all keys `K` posted
under a value `V`. Typically fed by a `FlatMap` that tokenizes documents into
`(document_key, term)` pairs.

```rust
FlatMap::new(
    |d: &Document| tokenize(d), // -> Vec<Keyed<usize, String>>
    terminal::InvertedIndex::new("words"),
)
```

Postings are **set-semantic per `Keyed<K, V>` pair**: a positive delta inserts the
posting, a non-positive delta deletes it, regardless of magnitude or how many
times it was previously inserted. (This differs from `Bag`, which counts.)
Practical consequence: if a document can contain the same term twice, a
single retraction of the document still removes the posting cleanly — but
don't rely on multiplicity semantics here.

Reader (`InvertedIndexReader<K, V>`):

| Method | Returns |
|---|---|
| `search(&q)` | `Vec<K>` — every key posted under exactly `q`; empty if none |

Lookups are prefix scans on the term's encoding, so they cost O(result set),
not O(index).

## Consistency across sinks

All readers handed to one `rtx` closure come from the same pinned snapshot.
If a pipeline maintains a `Count` and a `Bag` from the same stream, the count
you read will always equal the sum of the bag's multiplicities — there is no
window where one sink has absorbed a transaction and another hasn't.
