use super::manager::BlockStackInner;
use crate::stack::admission::RefreshToken;
use crate::stack::error::StackError;
use crate::stack::vma::{VmaSnapshot, read_vma_snapshot};

pub(super) fn reserve(inner: &BlockStackInner) -> Result<(), StackError> {
    let margin = match inner.safety_margin {
        Some(margin) => margin,
        None => return Ok(()),
    };
    let mut forced_refresh = false;
    loop {
        let mut state = inner.lock()?;
        let _hold = inner.timing.time(super::timing::TimingKind::LockHold);
        if state.admission.is_refreshing() {
            drop(_hold);
            state = inner.wait(state)?;
            drop(state);
            continue;
        }
        if state.admission.refresh_due() || forced_refresh {
            let token = match state.admission.begin_refresh() {
                Some(token) => token,
                None => {
                    drop(_hold);
                    drop(state);
                    continue;
                }
            };
            drop(_hold);
            drop(state);
            refresh_admission(inner, token)?;
            forced_refresh = false;
            continue;
        }
        match state.admission.check_cached(1, margin) {
            Ok(_) => {
                state.admission.begin_allocation()?;
                return Ok(());
            }
            Err(StackError::VmaPreflightRejected { .. }) if !forced_refresh => {
                drop(state);
                forced_refresh = true;
                continue;
            }
            Err(error) => return Err(error),
        }
    }
}

pub(super) fn refresh_snapshot(inner: &BlockStackInner) -> Result<VmaSnapshot, StackError> {
    loop {
        let mut state = inner.lock()?;
        let _hold = inner.timing.time(super::timing::TimingKind::LockHold);
        if state.admission.is_refreshing() {
            let generation = state.admission.snapshot_generation();
            drop(_hold);
            state = inner.wait(state)?;
            let snapshot = if state.admission.snapshot_generation() != generation {
                state.admission.snapshot()
            } else {
                None
            };
            drop(state);
            if let Some(snapshot) = snapshot {
                return Ok(snapshot);
            }
            continue;
        }
        let token = match state.admission.begin_refresh() {
            Some(token) => token,
            None => {
                drop(_hold);
                drop(state);
                continue;
            }
        };
        drop(_hold);
        drop(state);
        refresh_admission(inner, token)?;
        let state = inner.lock()?;
        let snapshot = state.admission.snapshot();
        drop(state);
        return snapshot.ok_or(StackError::MalformedVmaLimit);
    }
}

fn refresh_admission(inner: &BlockStackInner, token: RefreshToken) -> Result<(), StackError> {
    let observed = read_vma_snapshot();
    let mut state = inner.lock()?;
    let _hold = inner.timing.time(super::timing::TimingKind::LockHold);
    let result = state.admission.finish_refresh(observed, token);
    inner.shared.changed.notify_all();
    result
}

pub(super) fn cancel(inner: &BlockStackInner) -> Result<(), StackError> {
    if inner.safety_margin.is_none() {
        return Ok(());
    }
    let mut state = inner.lock()?;
    let _hold = inner.timing.time(super::timing::TimingKind::LockHold);
    state.admission.cancel_allocation()
}
