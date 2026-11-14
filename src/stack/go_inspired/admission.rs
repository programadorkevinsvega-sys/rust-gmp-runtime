use crate::stack::admission::VmaAdmission;
use crate::stack::error::StackError;
use crate::stack::vma::{VmaSnapshot, read_vma_snapshot};
use std::sync::{Condvar, Mutex, MutexGuard};

pub(super) struct Admission {
    state: Mutex<VmaAdmission>,
    changed: Condvar,
    margin: Option<u64>,
}

impl Admission {
    pub fn new(margin: Option<u64>) -> Self {
        Self {
            state: Mutex::new(VmaAdmission::default()),
            changed: Condvar::new(),
            margin,
        }
    }

    pub fn reserve_span(&self, slots: usize) -> Result<(), StackError> {
        let margin = match self.margin {
            Some(margin) => margin,
            None => return Ok(()),
        };
        let mut force_refresh = false;
        let mut rejection_refresh_used = false;
        loop {
            let mut state = self.lock()?;
            if state.is_refreshing() {
                state = self.wait(state)?;
                drop(state);
                continue;
            }
            if state.refresh_due() || force_refresh {
                let was_forced = force_refresh;
                let token = state.begin_refresh().ok_or(StackError::MetricsOverflow)?;
                drop(state);
                self.refresh(token)?;
                if was_forced {
                    force_refresh = false;
                }
                continue;
            }
            match state.check_cached(slots, margin) {
                Ok(_) => return reserve_pending_slots(&mut state, slots),
                Err(StackError::VmaPreflightRejected { .. }) if !rejection_refresh_used => {
                    rejection_refresh_used = true;
                    force_refresh = true;
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub fn finish_span(&self, slots: usize) -> Result<(), StackError> {
        if self.margin.is_none() {
            return Ok(());
        }
        let mut state = self.lock()?;
        for _ in 0..slots {
            state.finish_allocation()?;
        }
        Ok(())
    }

    pub fn cancel_span(&self, slots: usize) -> Result<(), StackError> {
        if self.margin.is_none() {
            return Ok(());
        }
        let mut state = self.lock()?;
        for _ in 0..slots {
            state.cancel_allocation()?;
        }
        Ok(())
    }

    pub fn begin_release(&self, slots: usize) -> Result<(), StackError> {
        if self.margin.is_none() {
            return Ok(());
        }
        let mut state = self.lock()?;
        let mut begun = 0_usize;
        while begun < slots {
            match state.begin_release() {
                Ok(()) => begun = begun.checked_add(1).ok_or(StackError::SizeOverflow)?,
                Err(original) => {
                    for _ in 0..begun {
                        if let Err(cleanup) = state.cancel_release() {
                            return Err(StackError::CleanupAfterFailure {
                                original: Box::new(original),
                                cleanup: Box::new(cleanup),
                            });
                        }
                    }
                    return Err(original);
                }
            }
        }
        Ok(())
    }

    pub fn finish_release(&self, slots: usize) -> Result<(), StackError> {
        if self.margin.is_none() {
            return Ok(());
        }
        let mut state = self.lock()?;
        for _ in 0..slots {
            state.finish_release()?;
        }
        Ok(())
    }

    pub fn cancel_release(&self, slots: usize) -> Result<(), StackError> {
        if self.margin.is_none() {
            return Ok(());
        }
        let mut state = self.lock()?;
        for _ in 0..slots {
            state.cancel_release()?;
        }
        Ok(())
    }

    pub fn snapshot(&self) -> Result<VmaSnapshot, StackError> {
        if self.margin.is_none() {
            return read_vma_snapshot();
        }
        loop {
            let mut state = self.lock()?;
            if state.is_refreshing() {
                let generation = state.snapshot_generation();
                state = self.wait(state)?;
                let snapshot = (state.snapshot_generation() != generation)
                    .then(|| state.snapshot())
                    .flatten();
                drop(state);
                if let Some(snapshot) = snapshot {
                    return Ok(snapshot);
                }
                continue;
            }
            let token = state.begin_refresh().ok_or(StackError::MetricsOverflow)?;
            drop(state);
            self.refresh(token)?;
            let state = self.lock()?;
            return state.snapshot().ok_or(StackError::MalformedVmaLimit);
        }
    }

    fn refresh(&self, token: crate::stack::admission::RefreshToken) -> Result<(), StackError> {
        let observed = read_vma_snapshot();
        let mut state = self.lock()?;
        let result = state.finish_refresh(observed, token);
        self.changed.notify_all();
        result
    }

    fn lock(&self) -> Result<MutexGuard<'_, VmaAdmission>, StackError> {
        self.state
            .lock()
            .map_err(|error| StackError::AdmissionPoisoned {
                message: error.to_string(),
            })
    }

    fn wait<'a>(
        &self,
        state: MutexGuard<'a, VmaAdmission>,
    ) -> Result<MutexGuard<'a, VmaAdmission>, StackError> {
        self.changed
            .wait(state)
            .map_err(|error| StackError::AdmissionPoisoned {
                message: error.to_string(),
            })
    }
}

fn reserve_pending_slots(state: &mut VmaAdmission, slots: usize) -> Result<(), StackError> {
    let mut reserved = 0_usize;
    while reserved < slots {
        match state.begin_allocation() {
            Ok(()) => reserved = reserved.checked_add(1).ok_or(StackError::SizeOverflow)?,
            Err(error) => {
                for _ in 0..reserved {
                    state.cancel_allocation()?;
                }
                return Err(error);
            }
        }
    }
    Ok(())
}
