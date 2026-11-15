//! Global overflow queue for work beyond the injection queue's fixed capacity.

use std::collections::VecDeque;

use crate::machine::worker::dispatch::DispatchTask;

#[derive(Default)]
pub(crate) struct GlobalRunQueue {
    tasks: VecDeque<DispatchTask>,
}

impl GlobalRunQueue {
    pub(crate) fn reserve_one(&mut self) -> Result<(), String> {
        self.tasks
            .try_reserve(1)
            .map_err(|error| format!("could not grow global run queue: {error}"))
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
