use super::{StackError, VmaSnapshot, parse_limit, parse_maps_count, read_file};

const MAP_LINE: &str = "00400000-00452000 r-xp 00000000 08:02 123 /bin/example";

#[test]
fn parses_vma_limit_and_rejects_invalid_values() {
    assert!(matches!(parse_limit("1048576\n"), Ok(1_048_576)));
    assert!(matches!(
        parse_limit(""),
        Err(StackError::MalformedVmaLimit)
    ));
    assert!(matches!(
        parse_limit("not-a-number"),
        Err(StackError::MalformedVmaLimit)
    ));
    assert!(matches!(
        parse_limit("0"),
        Err(StackError::MalformedVmaLimit)
    ));
    assert!(matches!(
        parse_limit("18446744073709551616"),
        Err(StackError::VmaLimitParse { .. })
    ));
}

#[test]
fn counts_valid_maps_regions_and_rejects_empty_or_malformed_content() {
    let contents = format!("{MAP_LINE}\n{MAP_LINE}\n");
    assert!(matches!(parse_maps_count(&contents), Ok(2)));
    assert!(matches!(
        parse_maps_count(&format!("{MAP_LINE}\r\n")),
        Ok(1)
    ));
    assert!(matches!(
        parse_maps_count(""),
        Err(StackError::MalformedVmaMaps { line: 0 })
    ));
    assert!(matches!(
        parse_maps_count("not a proc maps entry"),
        Err(StackError::MalformedVmaMaps { line: 1 })
    ));
    assert!(matches!(
        parse_maps_count(&format!("{MAP_LINE}\ninvalid\n")),
        Err(StackError::MalformedVmaMaps { line: 2 })
    ));
}

#[test]
fn rejects_malformed_fields_in_proc_maps_entries() {
    for line in [
        "-00452000 r-xp 00000000 08:02 123 /bin/example",
        "00400000- r-xp 00000000 08:02 123 /bin/example",
        "00400000-00452000 r-xp 00000000 08:02 /bin/example",
        "00452000-00400000 r-xp 00000000 08:02 123 /bin/example",
        "00400000-00452000 rwzp 00000000 08:02 123 /bin/example",
        "00400000-00452000 r-xp xyz 08:02 123 /bin/example",
        "00400000-00452000 r-xp 00000000 invalid 123 /bin/example",
    ] {
        assert!(matches!(
            parse_maps_count(line),
            Err(StackError::MalformedVmaMaps { line: 1 })
        ));
    }
}

#[test]
fn snapshot_clamps_remaining_and_reports_utilization() -> Result<(), StackError> {
    let snapshot = VmaSnapshot::from_counts(10, 12)?;
    assert_eq!(snapshot.remaining, 0);
    assert_eq!(snapshot.utilization_percent, 120.0);
    let available = VmaSnapshot::from_counts(10, 4)?;
    assert_eq!(available.remaining, 6);
    assert_eq!(available.utilization_percent, 40.0);
    assert!(matches!(
        VmaSnapshot::from_counts(0, 0),
        Err(StackError::MalformedVmaLimit)
    ));
    Ok(())
}

#[test]
fn procfs_read_errors_remain_explicit() {
    assert!(matches!(
        read_file("/proc/self/no-such-vma-file"),
        Err(StackError::VmaProcfs {
            path: "/proc/self/no-such-vma-file",
            ..
        })
    ));
}
