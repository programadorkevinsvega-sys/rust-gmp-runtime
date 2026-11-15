use super::*;

pub(super) fn parse_env<T>(name: &str, default: T) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + 'static,
{
    match env::var(name) {
        Ok(value) => Ok(value.parse()?),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn parse_list(name: &str, default: &str) -> Result<Vec<usize>, Box<dyn Error>> {
    let value = match env::var(name) {
        Ok(value) => value,
        Err(env::VarError::NotPresent) => default.to_owned(),
        Err(error) => return Err(error.into()),
    };
    let mut values = Vec::new();
    let mut seen = HashSet::new();
    for item in value.split(',') {
        let parsed = item.parse::<usize>()?;
        if !seen.insert(parsed) {
            return Err(format!("{name} contains duplicate value {parsed}").into());
        }
        values.push(parsed);
    }
    if values.is_empty() {
        return Err(format!("{name} must not be empty").into());
    }
    Ok(values)
}

pub(super) fn percentile_u128(values: &mut [u128], percentile: usize) -> u128 {
    values.sort_unstable();
    let index = values.len().saturating_mul(percentile).div_ceil(100);
    values[index.saturating_sub(1).min(values.len() - 1)]
}

pub(super) fn percentile_f64(values: &mut [f64], percentile: usize) -> f64 {
    values.sort_by(f64::total_cmp);
    let index = values.len().saturating_mul(percentile).div_ceil(100);
    values[index.saturating_sub(1).min(values.len() - 1)]
}

pub(super) fn percentile_u64(values: &mut [u64], percentile: usize) -> u64 {
    values.sort_unstable();
    let index = values.len().saturating_mul(percentile).div_ceil(100);
    values[index.saturating_sub(1).min(values.len() - 1)]
}

pub(super) fn median_absolute_deviation_u64(values: &[u64]) -> u64 {
    let median = percentile_u64(&mut values.to_vec(), 50);
    percentile_u64(
        &mut values
            .iter()
            .map(|value| value.abs_diff(median))
            .collect::<Vec<_>>(),
        50,
    )
}

pub(super) fn median_absolute_deviation_u128(values: &[u128]) -> u128 {
    let median = percentile_u128(&mut values.to_vec(), 50);
    percentile_u128(
        &mut values
            .iter()
            .map(|value| value.abs_diff(median))
            .collect::<Vec<_>>(),
        50,
    )
}

pub(super) fn median_absolute_deviation_f64(values: &[f64]) -> f64 {
    let median = percentile_f64(&mut values.to_vec(), 50);
    percentile_f64(
        &mut values
            .iter()
            .map(|value| (value - median).abs())
            .collect::<Vec<_>>(),
        50,
    )
}
