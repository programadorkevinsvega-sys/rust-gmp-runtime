use std::fs;
use std::io;

use super::error::StackError;

pub(super) struct ProcessMetrics {
    pub(super) virtual_bytes: Option<u64>,
    pub(super) resident_bytes: Option<u64>,
}

pub(super) fn read_process_metrics(page_size: usize) -> Result<ProcessMetrics, StackError> {
    let (virtual_bytes, resident_bytes) = read_process_memory(page_size)?;
    Ok(ProcessMetrics {
        virtual_bytes,
        resident_bytes,
    })
}

fn read_process_memory(page_size: usize) -> Result<(Option<u64>, Option<u64>), StackError> {
    const PATH: &str = "/proc/self/statm";

    let contents = match fs::read_to_string(PATH) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((None, None)),
        Err(source) => return Err(StackError::ProcessMetrics { path: PATH, source }),
    };

    // Linux documents statm's resident-page value as an approximation.
    let mut fields = contents.split_whitespace();
    let virtual_pages = parse_process_pages(fields.next(), PATH)?;
    let resident_pages = parse_process_pages(fields.next(), PATH)?;
    let page_size = match u64::try_from(page_size) {
        Ok(page_size) => page_size,
        Err(source) => {
            return Err(StackError::IntegerConversion {
                operation: "system page size",
                source,
            });
        }
    };
    let virtual_bytes = virtual_pages
        .checked_mul(page_size)
        .ok_or(StackError::MetricsOverflow)?;
    let resident_bytes = resident_pages
        .checked_mul(page_size)
        .ok_or(StackError::MetricsOverflow)?;
    Ok((Some(virtual_bytes), Some(resident_bytes)))
}

fn parse_process_pages(value: Option<&str>, path: &'static str) -> Result<u64, StackError> {
    let value = value.ok_or(StackError::MalformedProcessMetrics { path })?;
    match value.parse() {
        Ok(pages) => Ok(pages),
        Err(source) => Err(StackError::ProcessMetricsParse { path, source }),
    }
}

#[cfg(test)]
mod tests {
    use super::{StackError, parse_process_pages, read_process_metrics};

    #[test]
    fn reads_linux_process_metrics() -> Result<(), StackError> {
        let metrics = read_process_metrics(4096)?;
        if cfg!(target_os = "linux") {
            assert!(metrics.virtual_bytes.is_some());
            assert!(metrics.resident_bytes.is_some());
        }
        Ok(())
    }

    #[test]
    fn rejects_malformed_process_page_counts() {
        assert!(parse_process_pages(None, "/proc/self/statm").is_err());
        assert!(parse_process_pages(Some("bad"), "/proc/self/statm").is_err());
        assert!(matches!(
            parse_process_pages(Some("42"), "/proc/self/statm"),
            Ok(42)
        ));
    }
}
