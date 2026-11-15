use super::*;

#[test]
fn diagnostics_are_attributed_to_their_processor_shards() -> Result<(), String> {
    let store = SchedulerDiagnosticsStore::new(2);
    let processor_zero = store.processor(0)?;
    let processor_one = store.processor(1)?;
    SchedulerDiagnosticsStore::record_publication(processor_zero, 11, 13);
    SchedulerDiagnosticsStore::record_stolen_out(processor_one, 3);
    SchedulerDiagnosticsStore::record_active(processor_one, 17);
    let delta = ProcessorSchedulerDiagnostics {
        processor_index: 1,
        mutex_wait_ns: 19,
        mutex_hold_ns: 23,
        selected_tasks: 1,
        stolen_in: 3,
        ..ProcessorSchedulerDiagnostics::default()
    };
    SchedulerDiagnosticsStore::record_wait(processor_one, &delta);

    let snapshot = store.snapshot();
    assert_eq!(snapshot.mutex_wait_ns, 30);
    assert_eq!(snapshot.mutex_hold_ns, 36);
    assert_eq!(snapshot.processors[0].mutex_wait_ns, 11);
    assert_eq!(snapshot.processors[1].mutex_wait_ns, 19);
    assert_eq!(snapshot.processors[0].publish_mutex_wait_ns, 11);
    assert_eq!(snapshot.processors[1].worker_mutex_wait_ns, 19);
    assert_eq!(snapshot.processors[1].selected_tasks, 1);
    assert_eq!(snapshot.processors[1].stolen_in, 3);
    assert_eq!(snapshot.processors[1].stolen_out, 3);
    assert_eq!(snapshot.processors[1].active_ns, 17);
    Ok(())
}
