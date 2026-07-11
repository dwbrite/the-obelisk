# Writing your own operators

Everything in a pipeline — operators, sinks, tuples — implements one trait:
[`Push`](../src/pipeline/mod.rs). When the built-ins don't cover the derived
state you need, implement it yourself.

## The `Push` trait

```rust
pub trait Push<D: Clone> {
    /// Typed, lazy view over a pinned snapshot; only meaningful for sinks.
    type Reader<'tx, R: Readable + 'tx>;

    /// Resolve keyspace handles, register sink names. Once, at Stream::new.
    fn init(&mut self, init: &mut PipelineInitCtx<'_>);

    /// Accept one delta: +n inserts n copies of `data`, -n retracts them.
    fn push(&mut self, tx: &mut WriteTx<'_>, data: &D, delta: isize);

    /// Flush pending state; once as the transaction commits.
    fn commit(&mut self, tx: &mut WriteTx<'_>) {}

    /// Discard pending state; instead of commit if the transaction panics.
    fn abort(&mut self) {}

    /// Get a read handle over snapshot `tx`.
    fn reader<'tx, R: Readable>(&self, tx: &'tx R) -> Self::Reader<'tx, R>;
}
```

Lifecycle: `init` once when the owning `Stream` opens; then per write
transaction, `push` once per delta followed by exactly one of `commit`
(normal completion) or `abort` (the closure panicked).

## The contract

Whatever you build must uphold the invariants the rest of the system relies
on:

1. **Retraction cancels.** Pushing `(d, +n)` then `(d, -n)` must leave your
   state — and everything you emitted downstream — as if neither happened.
   This is *the* invariant; everything else serves it.
2. **Propagate the lifecycle.** An operator (a node with a downstream) must
   forward `init`, `commit`, `abort`, and `reader` to its `next` even if it
   holds no state itself. Forgetting to propagate `commit` silently drops all
   downstream buffered state.
3. **Buffer in `push`, reconcile in `commit`.** Stateful nodes should not
   read/write the store per push. Accumulate net deltas in a hash map keyed
   by the element's encoding; at commit, read the stored value once per
   touched key, apply the net delta, write back, and emit downstream deltas.
   `abort` just clears the buffer.
4. **Emit downstream before `next.commit`.** In your `commit`, push any
   resulting deltas into `self.next`, *then* call `self.next.commit(tx)` so
   downstream flushes what you just fed it.
5. **Names are schema.** If your node persists state, take a `name` in the
   constructor and claim the keyspace via `init.keyspace(&self.name)` — it
   panics on pipeline-wide duplicates, and the name is how state is found
   again on reopen.

## Anatomy of a stateless operator

Stateless operators are pure plumbing — transform in `push`, forward
everything else. `Filter` in its entirety:

```rust
pub struct Filter<D, F, G> {
    pub pred: F,
    pub next: G,
    _p: PhantomData<D>,
}

impl<F: Fn(&D) -> bool, G: Push<D>, D: Clone> Push<D> for Filter<D, F, G> {
    // transparent: expose the downstream's reader
    type Reader<'tx, R: Readable + 'tx> = G::Reader<'tx, R>;

    fn init(&mut self, init: &mut PipelineInitCtx<'_>) { self.next.init(init) }

    fn push(&mut self, tx: &mut WriteTx<'_>, data: &D, delta: isize) {
        if (self.pred)(data) {
            self.next.push(tx, data, delta) // delta passes through unchanged
        }
    }

    fn commit(&mut self, tx: &mut WriteTx<'_>) { self.next.commit(tx) }
    fn abort(&mut self) { self.next.abort() }

    fn reader<'tx, R: Readable>(&self, tx: &'tx R) -> Self::Reader<'tx, R> {
        self.next.reader(tx)
    }
}
```

Retraction is honored for free: the predicate is deterministic, so an insert
and its retraction make the same pass/drop decision.

## Anatomy of a stateful node

A sink follows the buffer/reconcile pattern. Sketch of `Bag`:

```rust
pub struct Bag<D> {
    name: String,
    ks: Option<fjall::SingleWriterTxKeyspace>,
    pending: FxHashMap<Vec<u8>, i64>, // encoded element -> net delta this tx
    _p: PhantomData<D>,
}

impl<D: Clone + Serialize + DeserializeOwned> Push<D> for Bag<D> {
    type Reader<'tx, R: Readable + 'tx> = BagReader<'tx, R, D>;

    fn init(&mut self, init: &mut PipelineInitCtx<'_>) {
        self.ks = Some(init.keyspace(&self.name)); // claims "sink_{name}"
    }

    fn push(&mut self, tx: &mut WriteTx<'_>, data: &D, delta: isize) {
        // serialize into the tx's shared scratch buffer — no per-push alloc
        tx.buf.clear();
        postcard::to_io(data, &mut tx.buf).unwrap();
        *self.pending.entry(tx.buf.clone()).or_insert(0) += delta as i64;
    }

    fn commit(&mut self, tx: &mut WriteTx<'_>) {
        let ks = self.ks.clone().unwrap();
        for (key, delta) in self.pending.drain() {
            if delta == 0 { continue; } // insert+remove in one tx: no-op
            let cur = tx.get(&ks, &key)
                .map(|v| i64::from_be_bytes(v.as_ref().try_into().unwrap()))
                .unwrap_or(0);
            let new = cur + delta;
            if new > 0 { tx.insert(&ks, &key, new.to_be_bytes()); }
            else       { tx.remove(&ks, &key); }
        }
    }

    fn abort(&mut self) { self.pending.clear(); }

    fn reader<'tx, R: Readable>(&self, tx: &'tx R) -> Self::Reader<'tx, R> {
        BagReader { tx, ks: self.ks.clone().unwrap(), _p: PhantomData }
    }
}
```

Things worth copying:

- **`tx.buf`** is a scratch `Vec<u8>` on the write transaction, shared by all
  nodes for serialization. Clear it, encode into it, then clone (or
  `mem::take` and restore) the bytes you need to keep.
- **Store encoded bytes as map keys**, not the data itself — postcard
  encodings are cheap to hash and compare, and they're what the store is
  keyed by anyway.
- **Delete on zero.** When a count/multiplicity reaches 0, remove the key so
  fully-retracted elements leave no residue.
- **`tx.get` sees the transaction's own uncommitted writes**, so multiple
  stateful nodes reconciling in the same commit compose correctly.

## Stateful operators that emit downstream

An operator like `Distinct` combines both patterns: buffer in `push`, and in
`commit` compare old vs. new stored state to decide what deltas downstream
should see (e.g. `+1` only when a count crosses 0 → positive). Study
[`Distinct`](../src/pipeline/ops/mod.rs) and
[`Aggregate`](../src/pipeline/ops/keyed.rs) — between them they demonstrate
edge-triggered emission and changelog emission (retract old value, insert new
value), which cover most derived-state shapes.

Note that `Distinct` buffers the *decoded* datum alongside the net delta so
it can re-emit downstream at commit; if your operator forwards data, you'll
need the same.

## Readers

If your node is a sink, define a reader struct holding `&'tx R` (the pinned
snapshot) plus your keyspace handle, and expose typed query methods on it.
The `Readable` trait (re-exported from fjall via `fold::stream`) provides
`get`, `contains_key`, `iter`, and `prefix` — see `BagReader` and
`InvertedIndexReader` in
[`terminal`](../src/pipeline/terminal/mod.rs) for the pattern, including the
thread-local key buffer trick that keeps point lookups allocation-free.
