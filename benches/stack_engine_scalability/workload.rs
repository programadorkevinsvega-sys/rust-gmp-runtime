#[derive(Clone, Copy, Debug)]
pub(super) enum Workload {
    Noop,
    Cpu64,
    Cpu2048,
}

impl Workload {
    pub(super) const ALL: [Self; 3] = [Self::Noop, Self::Cpu64, Self::Cpu2048];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::Cpu64 => "cpu64",
            Self::Cpu2048 => "cpu2048",
        }
    }

    pub(super) fn run(self, seed: u64) {
        let iterations = match self {
            Self::Noop => 0,
            Self::Cpu64 => 64,
            Self::Cpu2048 => 2_048,
        };
        let mut value = std::hint::black_box(seed);
        for _ in 0..iterations {
            value ^= value >> 12;
            value ^= value << 25;
            value ^= value >> 27;
            value = value.wrapping_mul(0x2545_f491_4f6c_dd1d);
        }
        std::hint::black_box(value);
    }
}
