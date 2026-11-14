//! Integration tests for routine context and scheduling observations.

use std::error::Error;

use rust_gmp_runtime::routine::{
    BlockingReason, Routine, RoutineCompletion, RoutineMetadata, RoutineSchedulingInfo,
    RoutineState, WaitReason,
};
use rust_gmp_runtime::stack::{StackConfig, StackManager};

fn make_routine() -> Result<Routine, Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(4096)?)?;
    Ok(Routine::new(&manager, RoutineMetadata::new())?)
}

#[test]
fn context_and_scheduling_metadata_follow_routine_lifecycle() -> Result<(), Box<dyn Error>> {
    let routine = make_routine()?;
    let bounds = routine.stack_bounds()?;
    let context = routine.context_info()?;
    assert_eq!(context.stack_upper_bound, bounds.usable_end);
    assert_eq!(context.saved_stack_pointer, None);
    assert_eq!(routine.scheduling_info()?, RoutineSchedulingInfo::default());

    routine.publish()?;
    let queued = routine.scheduling_info()?;
    assert_eq!(queued.runnable_entries, 1);
    assert!(queued.runnable_since.is_some());

    routine.try_start()?.wait_for(WaitReason::Io)?;
    let blocked = routine.scheduling_info()?;
    assert_eq!(
        blocked.blocking.map(|information| information.reason),
        Some(BlockingReason::Io)
    );
    assert!(blocked.running_since.is_none());

    routine.wake()?;
    assert_eq!(routine.scheduling_info()?.blocking, None);
    routine.try_start()?.begin_syscall()?;
    assert_eq!(
        routine
            .scheduling_info()?
            .blocking
            .map(|information| information.reason),
        Some(BlockingReason::ManagedSyscall)
    );
    routine.return_from_syscall()?;

    routine.try_start()?.yield_to_scheduler()?;
    let yielded = routine.scheduling_info()?;
    assert_eq!(yielded.execution_count, 3);
    assert_eq!(yielded.yield_count, 1);
    assert_eq!(yielded.runnable_entries, 4);

    routine.try_start()?.complete()?;
    let completed = routine.scheduling_info()?;
    assert_eq!(completed.execution_count, 4);
    assert!(completed.completed_at.is_some());
    assert!(completed.running_since.is_none());
    assert!(completed.blocking.is_none());
    Ok(())
}

#[test]
fn discard_is_terminal_before_or_after_publication() -> Result<(), Box<dyn Error>> {
    let created = make_routine()?;
    created.discard()?;
    assert_eq!(created.state()?, RoutineState::Dead);
    assert_eq!(created.completion()?, Some(RoutineCompletion::Abandoned));
    assert!(created.scheduling_info()?.completed_at.is_some());
    assert!(created.publish().is_err());

    let published = make_routine()?;
    published.publish()?;
    published.discard()?;
    assert_eq!(published.state()?, RoutineState::Dead);
    assert_eq!(published.scheduling_info()?.runnable_since, None);
    assert!(published.try_start().is_err());
    Ok(())
}

#[test]
fn each_runtime_wait_reason_is_preserved_in_blocking_snapshot() -> Result<(), Box<dyn Error>> {
    let reasons = [
        (WaitReason::Synchronization, BlockingReason::Synchronization),
        (WaitReason::Io, BlockingReason::Io),
        (WaitReason::Timer, BlockingReason::Timer),
        (WaitReason::External, BlockingReason::External),
    ];

    for (wait_reason, blocking_reason) in reasons {
        let routine = make_routine()?;
        routine.publish()?;
        routine.try_start()?.wait_for(wait_reason)?;
        assert_eq!(
            routine.scheduling_info()?.blocking.map(|info| info.reason),
            Some(blocking_reason)
        );
        routine.wake()?;
        routine.try_start()?.complete()?;
    }
    Ok(())
}
