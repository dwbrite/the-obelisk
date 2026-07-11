//! Pipeline operators and the [`Push`] trait that composes them.
//!
//! A pipeline is a tree of [`Push`] nodes built inside-out: every operator
//! owns its downstream (`next`), so the whole graph is one concrete type and
//! all calls are statically dispatched. Interior nodes transform or route
//! data; the leaves are [`terminal`] sinks that persist state.
//!
//! # Deltas
//! Data flows through the graph as `(data, delta)` pairs, where `delta` is a
//! signed multiplicity: `+n` inserts `n` copies of `data`, `-n` retracts
//! them. Every operator and sink honors retraction, which is what makes the
//! materialized state incrementally maintainable — pushing a record and later
//! pushing it again with the opposite delta leaves every sink unchanged.
//!
//! # Operators
//! Stateless operators forward transformed deltas immediately:
//! - [`Map`] — apply a function to each datum
//! - [`Filter`] — drop data failing a predicate
//! - [`FilterMap`] — map and filter in one step
//! - [`FlatMap`] — expand each datum into zero or more outputs
//!
//! Stateful operators persist per-element state in their own keyspace,
//! buffering within a transaction and emitting downstream deltas at commit:
//! - [`Distinct`] — collapse multiplicities to set semantics
//! - [`Aggregate`] — per-key incremental aggregation
//!
//! Keying operators convert between plain and [`Keyed`] streams:
//! - [`KeyBy`] — attach a key extracted from each datum
//! - [`Unkey`] — discard the key, forwarding the value
//!
//! # Fan-out
//! Tuples of `Push` nodes (up to 16 elements) implement `Push` by
//! broadcasting each delta to every element, splitting a pipeline into
//! parallel branches. The tuple's reader is the tuple of its elements'
//! readers.

use crate::stream::Readable;

use crate::stream::{PipelineInitCtx, WriteTx};

pub mod terminal;

mod ops;
pub use ops::*;

// contains (A,B,C..) tuples implementing Push for use as tee/tap
mod tuple;

/// A pipeline node that accepts a stream of `(data, delta)` pairs.
///
/// Implemented by operators (which transform and forward), sinks (which
/// persist), and tuples of nodes (which fan out). A node's lifecycle:
///
/// 1. [`init`](Push::init) — once, when the owning
///    [`Stream`](crate::stream::Stream) opens: resolve keyspaces, claim sink
///    names.
/// 2. [`push`](Push::push) — once per delta within a write transaction.
///    Stateful nodes typically buffer here rather than touching the store.
/// 3. [`commit`](Push::commit) — once as the transaction completes: flush
///    buffered state and emit any resulting downstream deltas.
/// 4. [`abort`](Push::abort) — instead of `commit` if the transaction
///    panics: discard buffered state so the node is clean for the next
///    transaction.
///
/// Operators must propagate `commit`/`abort`/`reader` to their downstream
/// node(s) even when they hold no state themselves.
pub trait Push<D: Clone> {
    /// Typed, lazy view over a pinned snapshot.
    /// Publically accessible in through read TXs; only relevant to sinks.
    /// Operators pass their downstream's reader through unchanged, so a
    /// pipeline's reader mirrors its sink structure.
    type Reader<'tx, R: Readable + 'tx>;

    /// Resolve keyspace handles and register sink names.
    fn init(&mut self, init: &mut PipelineInitCtx<'_>);

    /// Accept one delta: `+n` inserts `n` copies of `data`, `-n` retracts
    /// them.
    fn push(&mut self, tx: &mut WriteTx<'_>, data: &D, delta: isize);

    /// Flushes pending state to the store; called once as transaction completes.
    fn commit(&mut self, tx: &mut WriteTx<'_>) {
        let _ = tx;
    }

    /// Drop pending state to reset node for the next transaction.
    /// Called once if the transaction fails.
    fn abort(&mut self) {}

    /// Get a read handle to the sink.
    fn reader<'tx, R: Readable>(&self, tx: &'tx R) -> Self::Reader<'tx, R>;
}

impl<D: Clone, T: Push<D>> Push<D> for &mut T {
    type Reader<'tx, R: Readable + 'tx> = T::Reader<'tx, R>;
    #[inline]
    fn init(&mut self, init: &mut PipelineInitCtx<'_>) {
        (**self).init(init)
    }
    #[inline]
    fn push(&mut self, tx: &mut WriteTx<'_>, data: &D, delta: isize) {
        (**self).push(tx, data, delta)
    }
    #[inline]
    fn commit(&mut self, tx: &mut WriteTx<'_>) {
        (**self).commit(tx)
    }
    #[inline]
    fn abort(&mut self) {
        (**self).abort()
    }
    #[inline]
    fn reader<'tx, R: Readable>(&self, tx: &'tx R) -> Self::Reader<'tx, R> {
        (**self).reader(tx)
    }
}

/// A value paired with a grouping key.
///
/// The currency of keyed operators: [`KeyBy`] produces `Keyed` streams,
/// [`Aggregate`] consumes and emits them, and [`Unkey`] strips the key back
/// off.
#[derive(Clone)]
pub struct Keyed<K, V> {
    pub key: K,
    pub val: V,
}
impl<K, V> Keyed<K, V> {
    pub fn new(key: K, val: V) -> Self {
        Keyed { key, val }
    }

    /// Key `val` by a function of itself.
    pub fn new_by<F: Fn(&V) -> K>(val: V, key_fn: F) -> Self {
        Keyed {
            key: (key_fn)(&val),
            val,
        }
    }

    /// Discard the key.
    pub fn unkey(self) -> V {
        self.val
    }
}
