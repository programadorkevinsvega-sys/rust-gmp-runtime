//! Bounded queue for externally published runnable work.

use std::collections::VecDeque;

use crate::machine::worker::dispatch::DispatchTask;

pub(crate) const INJECTION_QUEUE_CAPACITY: usize = 1_024;

pub(crate) struct InjectionQueue {
    tasks: VecDeque<DispatchTask>,
}

impl InjectionQueue {
    pub(crate) fn new() -> Self {
        Self {
            tasks: VecDeque::with_capacity(INJECTION_QUEUE_CAPACITY),
        }
    }

    pub(crate) fn has_capacity(&self) -> bool {
        self.tasks.len() < INJECTION_QUEUE_CAPACITY
    }

    pub(crate) fn len(&self) -> usize {
        self.tasks.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    pub(crate) fn push_reserved(&mut self, task: DispatchTask) {
        self.tasks.push_back(task);
    }

    pub(crate) fn pop_front(&mut self) -> Option<DispatchTask> {
        self.tasks.pop_front()
    }
}
