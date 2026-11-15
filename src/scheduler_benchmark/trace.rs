use std::collections::BTreeMap;
use std::error::Error;

use super::types::TaskTransferTrace;
use crate::scheduler::StealEvent;

pub(super) fn build_task_traces(
    routine_ids: &[u64],
    events: Vec<StealEvent>,
) -> Result<Vec<TaskTransferTrace>, Box<dyn Error>> {
    let mut traces = BTreeMap::new();
    for routine_id in routine_ids {
        traces.insert(
            *routine_id,
            TaskTransferTrace {
                routine_id: *routine_id,
                initial_processor: 0,
                transfers: Vec::new(),
                selected_by: None,
            },
        );
    }
    let mut current_owners = routine_ids
        .iter()
        .map(|routine_id| (*routine_id, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for event in events {
        match event {
            StealEvent::Transferred {
                routine_id,
                from_processor,
                to_processor,
            } => {
                let owner = current_owners
                    .get_mut(&routine_id)
                    .ok_or("steal trace referenced an unknown routine")?;
                if *owner != from_processor {
                    return Err(format!(
                        "routine {routine_id} transfer source mismatch: expected P{owner}, got P{from_processor}"
                    )
                    .into());
                }
                *owner = to_processor;
                traces
                    .get_mut(&routine_id)
                    .ok_or("steal trace routine is missing")?
                    .transfers
                    .push((from_processor, to_processor));
            }
            StealEvent::Selected {
                routine_id,
                processor_index,
            } => {
                let owner = current_owners
                    .get(&routine_id)
                    .ok_or("selection trace referenced an unknown routine")?;
                if *owner != processor_index {
                    return Err(format!(
                        "routine {routine_id} selected by P{processor_index} while owned by P{owner}"
                    )
                    .into());
                }
                let trace = traces
                    .get_mut(&routine_id)
                    .ok_or("selection trace routine is missing")?;
                if trace.selected_by.replace(processor_index).is_some() {
                    return Err(format!("routine {routine_id} was selected more than once").into());
                }
            }
        }
    }
    if traces.values().any(|trace| trace.selected_by.is_none()) {
        return Err("steal trace contains a routine that was never selected".into());
    }
    Ok(traces.into_values().collect())
}

pub(super) fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("non-string panic payload")
        .to_owned()
}
