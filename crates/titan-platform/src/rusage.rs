//! Process resource counters used to prove properties of a measured region:
//! no page faults (memory is resident) and no involuntary context switches.
//!
//! macOS has no per-thread `getrusage`, so these are process-wide. Measured
//! regions keep all other threads idle, and results are reported as deltas.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub minor_faults: i64,
    pub major_faults: i64,
    pub voluntary_switches: i64,
    pub involuntary_switches: i64,
}

impl Usage {
    pub fn now() -> Usage {
        let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
        unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
        // No casts here. `c_long` is already `i64` on both targets this
        // work builds for, and a redundant cast warns on aarch64-apple-darwin
        // while being silently accepted on x86-64 Linux, which is where the
        // lint was being run.
        Usage {
            minor_faults: ru.ru_minflt,
            major_faults: ru.ru_majflt,
            voluntary_switches: ru.ru_nvcsw,
            involuntary_switches: ru.ru_nivcsw,
        }
    }

    pub fn since(&self, earlier: &Usage) -> Usage {
        Usage {
            minor_faults: self.minor_faults - earlier.minor_faults,
            major_faults: self.major_faults - earlier.major_faults,
            voluntary_switches: self.voluntary_switches - earlier.voluntary_switches,
            involuntary_switches: self.involuntary_switches - earlier.involuntary_switches,
        }
    }
}
