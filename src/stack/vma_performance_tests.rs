use std::error::Error;
use std::time::{Duration, Instant};

use super::error::StackError;
use super::vma::parse_maps_count;

const VMA_LINES: usize = 800_000;
const SAMPLES: usize = 10;
const MAP_LINE: &str = "00400000-00452000 r-xp 00000000 08:02 123\n";

#[test]
#[ignore = "manual synthetic comparison of VMA parser implementations"]
fn high_vma_parser_comparison() -> Result<(), Box<dyn Error>> {
    let capacity = MAP_LINE
        .len()
        .checked_mul(VMA_LINES)
        .ok_or("synthetic VMA fixture size overflow")?;
    let mut maps = String::with_capacity(capacity);
    for _ in 0..VMA_LINES {
        maps.push_str(MAP_LINE);
    }

    println!("vma_parser_comparison lines={VMA_LINES} samples={SAMPLES}");
    for sample in 0..SAMPLES {
        let (optimized, reference) = if sample % 2 == 0 {
            (
                measure(parse_maps_count, &maps)?,
                measure(parse_maps_count_reference, &maps)?,
            )
        } else {
            let reference = measure(parse_maps_count_reference, &maps)?;
            let optimized = measure(parse_maps_count, &maps)?;
            (optimized, reference)
        };
        println!(
            "vma_parser_comparison sample={sample} optimized_ns={} reference_ns={} speedup={:.3}",
            optimized.as_nanos(),
            reference.as_nanos(),
            reference.as_nanos() as f64 / optimized.as_nanos() as f64
        );
    }
    Ok(())
}

fn measure(
    parser: fn(&str) -> Result<u64, StackError>,
    maps: &str,
) -> Result<Duration, Box<dyn Error>> {
    let started = Instant::now();
    let count = parser(maps)?;
    let elapsed = started.elapsed();
    if count != VMA_LINES as u64 {
        return Err(format!("parser returned {count} lines, expected {VMA_LINES}").into());
    }
    Ok(elapsed)
}

fn parse_maps_count_reference(contents: &str) -> Result<u64, StackError> {
    if contents.is_empty() {
        return Err(StackError::MalformedVmaMaps { line: 0 });
    }
    let mut count = 0_u64;
    for (index, line) in contents.lines().enumerate() {
        if !valid_maps_line_reference(line) {
            let line = u64::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .ok_or(StackError::MetricsOverflow)?;
            return Err(StackError::MalformedVmaMaps { line });
        }
        count = count.checked_add(1).ok_or(StackError::MetricsOverflow)?;
    }
    if count == 0 {
        return Err(StackError::MalformedVmaMaps { line: 0 });
    }
    Ok(count)
}

fn valid_maps_line_reference(line: &str) -> bool {
    let mut fields = line.split_whitespace();
    let (Some(range), Some(permissions), Some(offset), Some(device), Some(inode)) = (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) else {
        return false;
    };
    let (start, end) = match range.split_once('-') {
        Some((start, end)) => (start, end),
        None => return false,
    };
    let addresses_valid = u64::from_str_radix(start, 16)
        .ok()
        .zip(u64::from_str_radix(end, 16).ok())
        .is_some_and(|(start, end)| start < end);
    let permissions_valid = permissions
        .bytes()
        .enumerate()
        .all(|(index, byte)| match index {
            0..=2 => byte == b'-' || matches!(byte, b'r' | b'w' | b'x'),
            3 => matches!(byte, b'p' | b's'),
            _ => false,
        });
    addresses_valid
        && permissions.len() == 4
        && permissions_valid
        && !offset.is_empty()
        && offset.bytes().all(|byte| byte.is_ascii_hexdigit())
        && device.split_once(':').is_some_and(|(major, minor)| {
            !major.is_empty()
                && !minor.is_empty()
                && major.bytes().all(|byte| byte.is_ascii_hexdigit())
                && minor.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        && !inode.is_empty()
        && inode.bytes().all(|byte| byte.is_ascii_digit())
}
