use std::marker::PhantomData;
use std::sync::{Mutex, MutexGuard};
use std::thread::{self, ThreadId};

use crate::routine::Routine;

use super::ProcessorError;
use super::id::ProcessorId;
use super::local_queue::LocalQueue;

/// Current association state of a Processor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessorState {
    /// Available for association with one worker thread.
    Unowned,
    /// Temporarily associated with the identified OS thread.
    Owned {
        /// OS thread currently holding the Processor binding.
        thread_id: ThreadId,
    },
}

#[derive(Debug, Default)]
struct ProcessorInner {
    owner: Option<ThreadId>,
    queue: LocalQueue,
}

/// Logical execution capacity with an exclusively accessed local run queue.
#[derive(Debug)]
pub struct Processor {
    id: ProcessorId,
    inner: Mutex<ProcessorInner>,
}

impl Processor {
    pub(super) fn new(id: ProcessorId) -> Self {
        Self {
            id,
            inner: Mutex::new(ProcessorInner::default()),
        }
    }

    /// Returns this Processor's pool-local identifier.
    pub fn id(&self) -> ProcessorId {
        self.id
    }

    /// Returns the current association state.
    ///
    /// # Errors
    ///
    /// Returns an error if the Processor lock is poisoned.
    pub fn state(&self) -> Result<ProcessorState, ProcessorError> {
        let inner = self.lock_inner()?;
        Ok(match inner.owner {
            Some(thread_id) => ProcessorState::Owned { thread_id },
            None => ProcessorState::Unowned,
        })
    }

    /// Exclusively associates this Processor with the calling OS thread.
    ///
    /// The binding is released when its guard is dropped. A guard cannot be
    /// sent to another thread.
    ///
    /// # Errors
    ///
    /// Returns an error if another binding exists or the lock is poisoned.
    pub fn bind_current(&self) -> Result<ProcessorBinding<'_>, ProcessorError> {
        let owner = thread::current().id();
        let mut inner = self.lock_inner()?;
        if inner.owner.is_some() {
            return Err(ProcessorError::AlreadyOwned);
        }
        inner.owner = Some(owner);
        drop(inner);
        Ok(ProcessorBinding {
            processor: self,
            owner,
            not_send: PhantomData,
        })
    }

    fn lock_inner(&self) -> Result<MutexGuard<'_, ProcessorInner>, ProcessorError> {
        self.inner
            .lock()
            .map_err(|error| ProcessorError::LockPoisoned(error.to_string()))
    }

    fn release(&self, owner: ThreadId) -> Result<(), ProcessorError> {
        let mut inner = self.lock_inner()?;
        if inner.owner != Some(owner) || owner != thread::current().id() {
            return Err(ProcessorError::NotOwner);
        }
        inner.owner = None;
        Ok(())
    }
}

/// Thread-bound access to a Processor and its local queue.
#[derive(Debug)]
pub struct ProcessorBinding<'processor> {
    processor: &'processor Processor,
    owner: ThreadId,
    not_send: PhantomData<*mut ()>,
}

impl ProcessorBinding<'_> {
    /// Returns the bound Processor.
    pub fn processor(&self) -> &Processor {
        self.processor
    }

    /// Adds a published routine to this Processor's local queue.
    ///
    /// # Errors
    ///
    /// Returns an error if the caller is not the owner, the routine is not
    /// `Runnable`, it is already queued, or the Processor lock is poisoned.
    pub fn enqueue(&mut self, routine: &Routine) -> Result<(), ProcessorError> {
        let mut inner = self.processor.lock_inner()?;
        self.verify_owner(&inner)?;
        inner.queue.push(routine)
    }

    /// Removes the oldest queued routine, if one is available.
    ///
    /// The caller must still claim the routine through its execution permit;
    /// its lifecycle may have changed since it entered the queue.
    ///
    /// # Errors
    ///
    /// Returns an error if the caller is not the owner or the lock is poisoned.
    pub fn dequeue(&mut self) -> Result<Option<Routine>, ProcessorError> {
        let mut inner = self.processor.lock_inner()?;
        self.verify_owner(&inner)?;
        inner.queue.pop()
    }

    /// Returns the number of routines waiting in the local queue.
    ///
    /// # Errors
    ///
    /// Returns an error if the caller is not the owner or the lock is poisoned.
    pub fn queue_len(&self) -> Result<usize, ProcessorError> {
        let inner = self.processor.lock_inner()?;
        self.verify_owner(&inner)?;
        Ok(inner.queue.len())
    }

    fn verify_owner(&self, inner: &ProcessorInner) -> Result<(), ProcessorError> {
        if self.owner != thread::current().id() || inner.owner != Some(self.owner) {
            return Err(ProcessorError::NotOwner);
        }
        Ok(())
    }
}

impl Drop for ProcessorBinding<'_> {
    fn drop(&mut self) {
        if let Err(error) = self.processor.release(self.owner) {
            eprintln!(
                "fatal: could not release Processor {}: {error}",
                self.processor.id.index()
            );
            std::process::abort();
        }
    }
}
