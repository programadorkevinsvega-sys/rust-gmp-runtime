pub(super) struct Summary {
    pub(super) work_units: usize,
    pub(super) submit: Vec<u128>,
    pub(super) post_submit_drain: Vec<u128>,
    pub(super) end_to_end: Vec<u128>,
    pub(super) dispatch_sum: Vec<u128>,
    pub(super) dispatch_max: Vec<u128>,
    pub(super) selected_by_processor: Vec<u64>,
    pub(super) peak_local_queues: Vec<usize>,
    pub(super) stolen_tasks: u64,
    pub(super) steal_attempts: u64,
    pub(super) failed_steal_attempts: u64,
}
