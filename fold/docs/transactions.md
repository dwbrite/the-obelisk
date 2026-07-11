# Transactions & persistence

All access to a `Stream` goes through transactions: `wtx` to write, `rtx` to
read. State lives in an embedded [fjall](https://docs.rs/fjall) LSM store —
single-writer, transactional, crash-safe.

## Write transactions: `wtx`

```rust
st.wtx(|tx| {
    tx.insert(&doc_a);          // delta +1
    tx.remove(&doc_b);          // delta -1
    tx.push(&doc_c, 3);         // explicit multiplicity
});
```

Every delta pushed inside one `wtx` closure commits **atomically** when the
closure returns: all sinks observe the whole batch or none of it. No reader
can ever see a state where the batch is half-applied across sinks.

`wtx` takes `&mut self` — there is exactly one writer.

### What happens during a transaction

1. Each `tx.push` drives the delta through the pipeline immediately.
   Stateless operators (`Map`, `Filter`, …) transform and forward on the
   spot; stateful nodes (`Distinct`, `Aggregate`) and sinks **buffer in
   memory** rather than touching the store.
2. When the closure returns, `commit` cascades down the pipeline: each
   stateful node reconciles its buffer against the store, emits any resulting
   downstream deltas (e.g. an aggregate's changelog), and the underlying
   fjall transaction commits.

Two practical consequences:

- **Batching is the main throughput lever.** Pushing the same hot key 10,000
  times in one `wtx` costs one store update at commit, not 10,000. Prefer one
  transaction per chunk of records over one per record.
- Emission from stateful nodes happens at commit, so ordering within a
  transaction is not observable — only the net effect is.

### Panics roll back

If the closure panics, the fjall transaction is dropped (rolling back all
store writes), every pipeline node's `abort` clears its buffered state, and
the panic resumes. The store and the pipeline are both clean for the next
transaction — a failed batch leaves no trace.

The crate is built with `panic = "abort"` in its own dev profile, but as a
library the rollback path is the contract: a panicking `wtx` closure must not
corrupt state.

## Read transactions: `rtx`

```rust
st.rtx(|(count, bag)| {
    // both readers see the same snapshot
    assert_eq!(count.get(), bag.iter().map(|(_, n)| n).sum::<i64>());
});
```

`rtx` pins one snapshot for the duration of the closure and hands you the
pipeline's reader. The reader's shape mirrors the pipeline's sink structure:
operators pass their downstream reader through unchanged, tuples yield tuples
of readers. A lone sink yields just its reader.

Because the snapshot spans all sinks, derived states are always mutually
consistent — you never observe sink A after a transaction and sink B before
it.

`rtx` takes `&self`; reads don't block each other and don't need the writer.

## Durability

There are two levels:

- **As soon as `wtx` returns**, the commit is durable against *process*
  crashes (kill -9, panic-abort). Reopening the store recovers every
  committed transaction.
- **`st.checkpoint()`** fsyncs all committed state to disk, additionally
  hardening it against OS crashes and power failure. Call it at whatever
  cadence your durability requirements dictate — after critical batches, on
  shutdown, or on a timer.

## Reopening a store

`Stream::new(path, pipeline)` opens the store at `path`, creating it if
absent. On open, `init` runs once through the pipeline: each named node
resolves its keyspace (partition `sink_{name}`) and names are
collision-checked (duplicates panic).

Because all sink and operator state is persistent and addressed by name,
reopening with the **same pipeline shape and the same names** resumes exactly
where the last committed transaction left off — ingest a million records,
restart the process, and the counts, bags, aggregates, and indexes are all
still there.

Corollaries:

- Renaming a node orphans its old state and starts it empty under the new
  name.
- Changing an operator's *function* (e.g. a different tokenizer in a
  `FlatMap`) does **not** migrate existing state. Retractions of previously
  inserted data would re-derive with the new function and fail to cancel.
  Treat a pipeline's functions as part of its persistent schema: to change
  them, start a fresh store (or fresh names) and re-ingest.
