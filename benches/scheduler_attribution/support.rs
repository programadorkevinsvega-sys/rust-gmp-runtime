use std::error::Error;

use rust_gmp_runtime::metrics::SchedulerStats;

pub(super) fn run_work(units: usize, seed: u64) {
    let mut value = seed ^ 0x9e37_79b9_7f4a_7c15;
    for _ in 0..units {
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        value = value.wrapping_mul(0x2545_f491_4f6c_dd1d);
    }
    std::hint::black_box(value);
}

pub(super) fn verify_stats(
    before: &SchedulerStats,
    after: &SchedulerStats,
    expected: usize,
) -> Result<(), Box<dyn Error>> {
    let expected = u64::try_from(expected)?;
    if after.published_tasks.saturating_sub(before.published_tasks) != expected
        || after.selected_tasks.saturating_sub(before.selected_tasks) != expected
        || after.completed_tasks.saturating_sub(before.completed_tasks) != expected
        || after
            .tasks_by_processor
            .iter()
            .zip(&before.tasks_by_processor)
            .map(|(after, before)| after.saturating_sub(*before))
            .sum::<u64>()
            != expected
    {
        return Err("exactly-once scheduler counters did not match submitted work".into());
    }
    Ok(())
}

pub(super) fn median(values: &mut [u128]) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

pub(super) fn format_counts<T: ToString>(values: &[T]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn parse_env<T>(name: &str, default: T) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + 'static,
{
    match std::env::var(name) {
        Ok(value) => Ok(value.parse()?),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn parse_list(name: &str, default: &str) -> Result<Vec<usize>, Box<dyn Error>> {
    let value = match std::env::var(name) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => default.to_owned(),
        Err(error) => return Err(error.into()),
    };
    let values = value
        .split(',')
        .map(str::parse)
        .collect::<Result<Vec<usize>, _>>()?;
    if values.is_empty() {
        return Err(format!("{name} must not be empty").into());
    }
    Ok(values)
}

pub(super) fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("non-string panic payload")
        .to_owned()
}
