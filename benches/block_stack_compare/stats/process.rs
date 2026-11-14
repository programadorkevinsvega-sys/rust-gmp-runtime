use std::error::Error;
use std::fs;

pub(super) fn process_memory(
    page_size: usize,
) -> Result<(Option<u64>, Option<u64>), Box<dyn Error>> {
    let contents = fs::read_to_string("/proc/self/statm")?;
    let mut fields = contents.split_whitespace();
    let virtual_pages: u64 = fields
        .next()
        .ok_or("statm virtual count missing")?
        .parse()?;
    let resident_pages: u64 = fields
        .next()
        .ok_or("statm resident count missing")?
        .parse()?;
    let page_size = u64::try_from(page_size)?;
    let virtual_bytes = virtual_pages
        .checked_mul(page_size)
        .ok_or("virtual bytes overflow")?;
    let rss_bytes = resident_pages
        .checked_mul(page_size)
        .ok_or("RSS bytes overflow")?;
    Ok((Some(virtual_bytes), Some(rss_bytes)))
}
