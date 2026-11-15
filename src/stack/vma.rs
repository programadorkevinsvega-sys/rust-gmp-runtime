use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};

use super::error::StackError;

const LIMIT_PATH: &str = "/proc/sys/vm/max_map_count";
pub(super) const MAPS_PATH: &str = "/proc/self/maps";
const MAPS_BUFFER_BYTES: usize = 64 * 1024;

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
    let mut reader = BufReader::with_capacity(MAPS_BUFFER_BYTES, file);
    let mut line = Vec::with_capacity(256);
    let mut count = 0_u64;
    loop {
        line.clear();
        let bytes =
            reader
                .read_until(b'\n', &mut line)
                .map_err(|source| StackError::VmaProcfs {
                    path: MAPS_PATH,
                    source,
                })?;
        if bytes == 0 {
            break;
        }
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        while line.last() == Some(&b'\r') {
            line.pop();
        }
        std::str::from_utf8(&line).map_err(|error| StackError::VmaProcfs {
            path: MAPS_PATH,
            source: io::Error::new(io::ErrorKind::InvalidData, error),
        })?;
        if !valid_maps_line_bytes(&line) {
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
        if !valid_maps_line_bytes(line.as_bytes()) {
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

fn valid_maps_line_bytes(bytes: &[u8]) -> bool {
    let mut position = 0;
    let (Some(range), Some(permissions), Some(offset), Some(device), Some(inode)) = (
        next_field(bytes, &mut position),
        next_field(bytes, &mut position),
        next_field(bytes, &mut position),
        next_field(bytes, &mut position),
        next_field(bytes, &mut position),
    ) else {
        return false;
    };
    let (start, end) = match range.iter().position(|byte| *byte == b'-') {
        Some(separator) => (&range[..separator], &range[separator + 1..]),
        None => return false,
    };
    let addresses_valid = parse_hex_u64(start)
        .zip(parse_hex_u64(end))
        .is_some_and(|(start, end)| start < end);
    let permissions_valid = permissions.len() == 4
        && (permissions[0] == b'-' || permissions[0] == b'r')
        && (permissions[1] == b'-' || permissions[1] == b'w')
        && (permissions[2] == b'-' || permissions[2] == b'x')
        && matches!(permissions[3], b'p' | b's');
    addresses_valid
        && permissions_valid
        && !offset.is_empty()
        && offset.iter().all(u8::is_ascii_hexdigit)
        && device
            .iter()
            .position(|byte| *byte == b':')
            .is_some_and(|separator| {
                let major = &device[..separator];
                let minor = &device[separator + 1..];
                !major.is_empty()
                    && !minor.is_empty()
                    && major.iter().all(u8::is_ascii_hexdigit)
                    && minor.iter().all(u8::is_ascii_hexdigit)
            })
        && !inode.is_empty()
        && inode.iter().all(u8::is_ascii_digit)
}

fn next_field<'a>(line: &'a [u8], position: &mut usize) -> Option<&'a [u8]> {
    while *position < line.len() && line[*position].is_ascii_whitespace() {
        *position += 1;
    }
    let start = *position;
    while *position < line.len() && !line[*position].is_ascii_whitespace() {
        *position += 1;
    }
    (start < *position).then_some(&line[start..*position])
}

fn parse_hex_u64(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() {
        return None;
    }
    bytes.iter().try_fold(0_u64, |value, byte| {
        let digit = match byte {
            b'0'..=b'9' => u64::from(byte - b'0'),
            b'a'..=b'f' => u64::from(byte - b'a' + 10),
            b'A'..=b'F' => u64::from(byte - b'A' + 10),
            _ => return None,
        };
        value.checked_mul(16)?.checked_add(digit)
    })
}

#[cfg(test)]
#[path = "vma_tests.rs"]
mod tests;
