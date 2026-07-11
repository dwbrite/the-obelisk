# Concepts

Fold has a small number of load-bearing ideas. Once they click, the whole API
reads naturally.

## Deltas: data with a sign

The unit of input is not a record — it is a **delta**: a datum paired with a
signed multiplicity.

```text
("hello", +1)   // insert one copy of "hello"
("hello", +3)   // insert three copies
("hello", -1)   // retract one copy
```

`tx.insert(&d)` and `tx.remove(&d)` are just sugar for `tx.push(&d, 1)` and
`tx.push(&d, -1)`.

This is the key to incremental maintenance. Every operator and every sink is
written to honor negative deltas: pushing a record and later pushing the same
record with the opposite sign leaves all downstream state exactly as if the
record had never existed. Deleting a document from an inverted index doesn't
rebuild the index — the retraction flows through the same pipeline and
subtracts the document's postings.

Because retraction is "re-derive and negate", **stateless operator functions
must be deterministic**. When you remove a datum, `Map`'s function runs again
on the original input and must produce the same output that was inserted;
otherwise downstream state won't cancel. Don't close over mutable state,
clocks, or randomness.

## Pipelines: a tree of operators, built inside-out

A pipeline is a tree of nodes implementing the
[`Push`](../src/pipeline/mod.rs) trait. Each operator *owns* its downstream
node (`next`), so you construct the pipeline from the leaves up:

```rust
Filter::new(
    |s: &String| !s.is_empty(),        // 1st: filter…
    Map::new(
        |s: &String| s.len(),          // 2nd: …then map…
        terminal::Bag::new("lengths"), // 3rd: …into a bag.
    ),
)
```

Read it outside-in: data enters `Filter`, survivors go to `Map`, results land
in the `Bag`. There is no dynamic dispatch anywhere — the whole tree is one
concrete type, and every `push` call is statically dispatched and inlinable.

### Fan-out with tuples

A tuple of nodes (up to 16 elements) is itself a node: it broadcasts every
delta to each element. This is how one stream feeds several branches:

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

## Sinks: the leaves that persist

Interior nodes transform and route; **terminal sinks** persist. The built-ins
live in `fold::pipeline::terminal`:

- `Count` — the number of live records (a running sum of deltas).
- `Bag` — a counted multiset: each distinct element and its multiplicity.
- `InvertedIndex` — postings of `(key, term)` pairs with exact-match search.

Every sink (and every stateful operator) takes a **name** at construction.
The name identifies its keyspace in the store and must be unique across the
whole pipeline — `Stream::new` panics on duplicates. Names are also how state
survives restarts: reopen the same path with the same pipeline shape and
names, and every sink picks up exactly where it left off.

## Readers mirror the pipeline

You never query sinks directly; you open a read transaction and get a
**reader** whose shape mirrors the pipeline's sink structure. Operators are
transparent — they pass their downstream's reader through — and tuples yield
tuples:

```rust
// pipeline: Filter → (Count, Bag, Map → Bag)
st.rtx(|(count, raw, uppers)| {
    println!("{} live records", count.get());
    for (s, n) in raw.iter() { /* … */ }
});
```

All readers in one `rtx` observe the **same pinned snapshot**, so the sinks
are always mutually consistent: you will never see a count that disagrees
with the bag it was derived alongside.

## Transactions: batches in, snapshots out

- `st.wtx(|tx| { … })` runs a **write transaction**. Every delta pushed
  inside commits atomically when the closure returns — all sinks observe the
  whole batch or none of it. If the closure panics, everything rolls back.
- `st.rtx(|readers| { … })` runs a **read transaction** over one snapshot.

Stateful nodes buffer their updates in memory during a transaction and flush
once at commit, so pushing the same hot key a thousand times in one batch
touches the store roughly once.

See [Transactions & persistence](transactions.md) for durability details.

## Keyed streams

Grouping is expressed in the type system. `KeyBy` turns a stream of `V` into
a stream of `Keyed<K, V>`; `Aggregate` consumes `Keyed<K, V>` and emits
`Keyed<K, A>` (the accumulator); `Unkey` strips the key back off. See
[Operators](operators.md#keyed-operators).

## What Fold is not

- **Not a query engine.** There is no ad-hoc query language; you declare the
  derived state you'll need up front, as pipeline structure.
- **Not distributed.** State lives in one embedded
  [fjall](https://docs.rs/fjall) LSM store, single-writer.
- **Not lazy.** Work happens at write time so reads are cheap — the classic
  materialized-view trade.
