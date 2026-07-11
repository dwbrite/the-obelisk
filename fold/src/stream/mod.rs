//! The [`Stream`] driver and transaction plumbing.
//!
//! [`Stream`] owns a pipeline and its backing store, and mediates all access
//! through transactions: [`Stream::wtx`] atomically feeds a batch of deltas
//! through the pipeline, [`Stream::rtx`] reads every sink from one pinned
//! snapshot.

pub use fjall::Readable;
use std::marker::PhantomData;

mod unkeyed;
use fxhash::FxHashSet;
pub use unkeyed::*;

use crate::pipeline::Push;

/// Write handle passed to [`Stream::wtx`] closures.
///
/// Each call feeds one delta into the head of the pipeline. All deltas
/// pushed within a single `wtx` commit atomically.
pub struct Tx<'g, 'tx, D: Clone, P: Push<D>> {
    pipeline: &'g mut P,
    tx: &'g mut WriteTx<'tx>,
    _p: PhantomData<D>,
}

impl<D: Clone, P: Push<D>> Tx<'_, '_, D, P> {
    /// Push `data` with an explicit signed multiplicity: `+n` inserts `n`
    /// copies, `-n` retracts them.
    #[inline]
    pub fn push(&mut self, data: &D, delta: isize) {
        self.pipeline.push(self.tx, data, delta);
    }
    /// Push one copy of `data` (delta `+1`).
    #[inline]
    pub fn insert(&mut self, data: &D) {
        self.push(data, 1);
    }
    /// Retract one copy of `data` (delta `-1`).
    #[inline]
    pub fn remove(&mut self, data: &D) {
        self.push(data, -1);
    }
}

/// Passed through the graph once at startup. Resolves named keyspaces and
/// collision-checks sink names.
pub struct PipelineInitCtx<'a> {
    store: &'a fjall::SingleWriterTxDatabase,
    taken: FxHashSet<String>,
}
impl PipelineInitCtx<'_> {
    pub fn new(store: &fjall::SingleWriterTxDatabase) -> PipelineInitCtx<'_> {
        PipelineInitCtx {
            store,
            taken: FxHashSet::default(),
        }
    }

    /// Open (or create) the keyspace for the named node, backing its state
    /// with the partition `sink_{name}`.
    ///
    /// # Panics
    /// Panics if `name` was already claimed by another node in this
    /// pipeline.
    pub fn keyspace(&mut self, name: &str) -> fjall::SingleWriterTxKeyspace {
        assert!(
            self.taken.insert(name.to_string()),
            "duplicate sink name: {name}"
        );
        self.store
            .keyspace(
                format!("sink_{name}").as_str(),
                fjall::KeyspaceCreateOptions::default,
            )
            .unwrap()
    }
}

/// A store write transaction threaded through the pipeline during a
/// [`Stream::wtx`].
///
/// Wraps the underlying fjall transaction with a reusable scratch buffer
/// (`buf`) that nodes borrow for serialization instead of allocating per
/// push.
pub struct WriteTx<'a> {
    tx: fjall::SingleWriterWriteTx<'a>,
    pub buf: Vec<u8>, // reusable buffer
}

impl WriteTx<'_> {
    pub fn new(tx: fjall::SingleWriterWriteTx<'_>) -> WriteTx<'_> {
        WriteTx {
            tx,
            buf: Vec::with_capacity(64),
        }
    }

    #[inline]
    pub fn insert(
        &mut self,
        ks: &fjall::SingleWriterTxKeyspace,
        k: impl AsRef<[u8]>,
        v: impl AsRef<[u8]>,
    ) {
        self.tx.insert(ks, k.as_ref(), v.as_ref());
    }

    #[inline]
    pub fn remove(&mut self, ks: &fjall::SingleWriterTxKeyspace, k: impl AsRef<[u8]>) {
        self.tx.remove(ks, k.as_ref());
    }

    /// Read a key, seeing this transaction's own uncommitted writes.
    #[inline]
    pub fn get(
        &mut self,
        ks: &fjall::SingleWriterTxKeyspace,
        k: impl AsRef<[u8]>,
    ) -> Option<fjall::Slice> {
        self.tx.get(ks, k).unwrap()
    }

    pub fn commit(self) {
        self.tx.commit().unwrap()
    }
}
