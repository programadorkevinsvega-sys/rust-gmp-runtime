#[derive(Clone, Copy, Debug)]
pub(super) enum Workload {
    Noop,
    CpuLight,
    CpuModerate,
    CpuHeavy,
}

impl Workload {
    pub(super) const ALL: [Self; 4] = [
        Self::Noop,
        Self::CpuLight,
        Self::CpuModerate,
        Self::CpuHeavy,
    ];

    pub(super) fn index(self) -> usize {
        match self {
            Self::Noop => 0,
            Self::CpuLight => 1,
            Self::CpuModerate => 2,
            Self::CpuHeavy => 3,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::CpuLight => "cpu_light_64",
            Self::CpuModerate => "cpu_moderate_2048",
            Self::CpuHeavy => "cpu_heavy_65536",
        }
    }

    pub(super) fn execute(self) {
        self.execute_with_seed(0x9e37_79b9_u64);
    }

    pub(super) fn execute_with_seed(self, seed: u64) {
        let iterations = match self {
            Self::Noop => 0,
            Self::CpuLight => 64,
            Self::CpuModerate => 2_048,
            Self::CpuHeavy => 65_536,
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
