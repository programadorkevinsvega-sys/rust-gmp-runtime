use super::manager::{BlockStack, BlockStackInner};
use super::release::release_slot;
use super::timing::TimingKind;

impl Drop for BlockStack {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        match release_slot(&self.inner, self.arena_index, self.slot_index) {
            Ok(()) => self.released = true,
            Err(error) => {
                eprintln!("fatal: failed to release experimental block stack: {error}");
                std::process::abort();
            }
        }
    }
}

impl Drop for BlockStackInner {
    fn drop(&mut self) {
        let state = match self.shared.state.get_mut() {
            Ok(state) => state,
            Err(error) => {
                eprintln!("fatal: block allocator state is poisoned during cleanup: {error}");
                std::process::abort();
            }
        };
        for arena in &mut state.arenas {
            let _timer = self.timing.time(TimingKind::Munmap);
            if let Err(error) = arena.mapping.release() {
                eprintln!("fatal: failed to release experimental block arena: {error}");
                std::process::abort();
            }
        }
    }
}
