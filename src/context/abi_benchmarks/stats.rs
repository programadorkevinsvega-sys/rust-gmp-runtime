pub(super) fn median(sorted_samples: &[f64]) -> f64 {
    let middle = sorted_samples.len() / 2;
    if sorted_samples.len() % 2 == 0 {
        (sorted_samples[middle - 1] + sorted_samples[middle]) / 2.0
    } else {
        sorted_samples[middle]
    }
}
