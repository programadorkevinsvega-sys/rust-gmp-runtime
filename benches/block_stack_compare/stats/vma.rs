use std::fs;
use std::io::{BufRead, BufReader};

pub fn current_vma_count() -> Result<u64, Box<dyn Error>> {
    let file = fs::File::open("/proc/self/maps")?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut count = 0_u64;
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        count = count.checked_add(1).ok_or("VMA count overflow")?;
    }
    if count == 0 {
        return Err("procfs reported no process mappings".into());
    }
    Ok(count)
}
