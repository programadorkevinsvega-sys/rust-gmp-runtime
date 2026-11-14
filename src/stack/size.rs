use super::error::StackError;

pub(super) fn round_up_to_page(requested: usize, page_size: usize) -> Result<usize, StackError> {
    let adjusted = requested
        .checked_add(page_size - 1)
        .ok_or(StackError::SizeOverflow)?;
    Ok((adjusted / page_size) * page_size)
}

#[cfg(test)]
mod tests {
    use super::round_up_to_page;
    use crate::stack::StackError;

    #[test]
    fn page_rounding_checks_overflow_and_alignment() {
        assert!(matches!(round_up_to_page(1, 4096), Ok(4096)));
        assert!(matches!(round_up_to_page(4096, 4096), Ok(4096)));
        assert!(matches!(round_up_to_page(4097, 4096), Ok(8192)));
        assert!(matches!(
            round_up_to_page(usize::MAX, 4096),
            Err(StackError::SizeOverflow)
        ));
    }
}
