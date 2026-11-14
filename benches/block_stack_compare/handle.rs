#![allow(unsafe_code)]

use rust_gmp_runtime::stack::{BlockStack, GoInspiredStack, Stack};

pub enum Handle {
    Fixed(Stack),
    Block(BlockStack),
    GoInspired(GoInspiredStack),
}

impl Handle {
    pub fn touch(&self) {
        let address = match self {
            Self::Fixed(stack) => stack.bounds().usable_start,
            Self::Block(stack) => stack.bounds().1,
            Self::GoInspired(stack) => stack.bounds().usable_start,
        };
        // SAFETY: Each handle owns writable usable pages until it is dropped.
        unsafe { std::ptr::write_volatile(address as *mut u8, 1) };
    }

    pub fn touch_all_pages(&self, page_size: usize) {
        let (start, end) = match self {
            Self::Fixed(stack) => {
                let bounds = stack.bounds();
                (bounds.usable_start, bounds.usable_end)
            }
            Self::Block(stack) => {
                let (_, start, end) = stack.bounds();
                (start, end)
            }
            Self::GoInspired(stack) => {
                let bounds = stack.bounds();
                (bounds.usable_start, bounds.usable_end)
            }
        };
        for address in (start..end).step_by(page_size) {
            // SAFETY: The live handle owns every writable page in this range.
            unsafe { std::ptr::write_volatile(address as *mut u8, 1) };
        }
    }
}
