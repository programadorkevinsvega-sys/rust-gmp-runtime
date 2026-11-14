use std::fs::{self, File};
use std::io::{BufRead, BufReader};

use super::error::StackError;

const LIMIT_PATH: &str = "/proc/sys/vm/max_map_count";
pub(super) const MAPS_PATH: &str = "/proc/self/maps";

/// One Linux procfs observation of the process VMA limit and current count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VmaSnapshot {
    /// Configured kernel VMA ceiling read from `/proc/sys/vm/max_map_count`.
    pub limit: u64,
    /// Number of valid regions listed by `/proc/self/maps`.
    pub current: u64,
    /// Theoretical remaining capacity, clamped to zero when already over limit.
    pub remaining: u64,
    /// Current count divided by the limit; values above 100 indicate overage.
    pub utilization_percent: f64,
}

impl VmaSnapshot {
    pub(super) fn from_counts(limit: u64, current: u64) -> Result<Self, StackError> {
        if limit == 0 {
            return Err(StackError::MalformedVmaLimit);
        }
        Ok(Self {
            limit,
            current,
            remaining: limit.saturating_sub(current),
            utilization_percent: current as f64 * 100.0 / limit as f64,
        })
    }
}

pub(super) fn read_vma_snapshot() -> Result<VmaSnapshot, StackError> {
    let limit = parse_limit(&read_file(LIMIT_PATH)?)?;
    let current = read_vma_count()?;
    VmaSnapshot::from_counts(limit, current)
}

fn read_file(path: &'static str) -> Result<String, StackError> {
    fs::read_to_string(path).map_err(|source| StackError::VmaProcfs { path, source })
}

pub(super) fn read_vma_count() -> Result<u64, StackError> {
    let file = File::open(MAPS_PATH).map_err(|source| StackError::VmaProcfs {
        path: MAPS_PATH,
        source,
    })?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut count = 0_u64;
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|source| StackError::VmaProcfs {
                path: MAPS_PATH,
                source,
            })?;
        if bytes == 0 {
            break;
        }
        let content = line.trim_end_matches(['\n', '\r']);
        if !valid_maps_line(content) {
            return Err(StackError::MalformedVmaMaps {
                line: count.checked_add(1).ok_or(StackError::MetricsOverflow)?,
            });
        }
        count = count.checked_add(1).ok_or(StackError::MetricsOverflow)?;
    }
    if count == 0 {
        return Err(StackError::MalformedVmaMaps { line: 0 });
    }
    Ok(count)
}

fn parse_limit(contents: &str) -> Result<u64, StackError> {
    let value = contents.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(StackError::MalformedVmaLimit);
    }
    let limit = value
        .parse()
        .map_err(|source| StackError::VmaLimitParse { source })?;
    if limit == 0 {
        return Err(StackError::MalformedVmaLimit);
    }
    Ok(limit)
}

#[cfg(test)]
pub(super) fn parse_maps_count(contents: &str) -> Result<u64, StackError> {
    if contents.is_empty() {
        return Err(StackError::MalformedVmaMaps { line: 0 });
    }
    let mut count = 0_u64;
    for (index, line) in contents.lines().enumerate() {
        if !valid_maps_line(line) {
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

fn valid_maps_line(line: &str) -> bool {
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

#[cfg(test)]
#[path = "vma_tests.rs"]
mod tests;
