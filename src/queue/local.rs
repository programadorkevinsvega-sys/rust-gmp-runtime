//! Per-Processor runnable queue.

use std::collections::VecDeque;

use crate::machine::worker::dispatch::DispatchTask;

pub(crate) const LOCAL_QUEUE_CAPACITY: usize = 256;

pub(crate) struct LocalRunQueue {
    tasks: VecDeque<DispatchTask>,
}

impl LocalRunQueue {
    pub(crate) fn new() -> Self {
        Self {
            tasks: VecDeque::with_capacity(LOCAL_QUEUE_CAPACITY),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.tasks.len()
    }

    pub(crate) fn remaining_capacity(&self) -> usize {
        LOCAL_QUEUE_CAPACITY.saturating_sub(self.tasks.len())
    }

    pub(crate) fn push_reserved(&mut self, task: DispatchTask) {
        self.tasks.push_back(task);
    }

    pub(crate) fn pop_front(&mut self) -> Option<DispatchTask> {
        self.tasks.pop_front()
    }

    pub(crate) fn pop_back(&mut self) -> Option<DispatchTask> {
        self.tasks.pop_back()
    }
}
