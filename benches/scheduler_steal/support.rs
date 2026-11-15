use std::error::Error;

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
    let mut values = Vec::new();
    for item in value.split(',') {
        let parsed = item.parse::<usize>()?;
        if values.contains(&parsed) {
            return Err(format!("{name} contains duplicate value {parsed}").into());
        }
        values.push(parsed);
    }
    if values.is_empty() {
        return Err(format!("{name} must not be empty").into());
    }
    Ok(values)
}

pub(super) fn percentile(values: &mut [u128], percentile: usize) -> u128 {
    values.sort_unstable();
    let index = values.len().saturating_mul(percentile).div_ceil(100);
    values[index.saturating_sub(1).min(values.len() - 1)]
}

pub(super) fn median_absolute_deviation(values: &[u128]) -> u128 {
    let median = percentile(&mut values.to_vec(), 50);
    percentile(
        &mut values
            .iter()
            .map(|value| value.abs_diff(median))
            .collect::<Vec<_>>(),
        50,
    )
}
