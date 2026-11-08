//! CPU pinning of a run: the server on one set of CPUs, the load generator on another, so the
//! two do not compete for a core (`docs/quality.md`, row 3.3).

const SET_BYTES: usize = 128;

unsafe extern "C" {
    fn sched_setaffinity(pid: i32, size: usize, mask: *const u8) -> i32;
}

/// A CPU list as `taskset -c` and `/sys/devices/system/cpu/online` write it: `0-3,8`.
pub fn parse_cpus(text: &str) -> Result<Vec<usize>, String> {
    let mut cpus = Vec::new();
    for part in text.split(',') {
        let bad = || format!("bad CPU list {text:?}");
        let (low, high) = match part.split_once('-') {
            Some((low, high)) => (
                low.parse::<usize>().map_err(|_| bad())?,
                high.parse::<usize>().map_err(|_| bad())?,
            ),
            None => {
                let cpu = part.parse::<usize>().map_err(|_| bad())?;
                (cpu, cpu)
            }
        };
        if low > high || high >= SET_BYTES * 8 {
            return Err(bad());
        }
        cpus.extend(low..=high);
    }
    Ok(cpus)
}

/// Pin the calling thread (and every thread and process it starts after) to `cpus`.
pub fn pin(cpus: &[usize]) -> Result<(), String> {
    let mut mask = [0u8; SET_BYTES];
    for cpu in cpus {
        mask[cpu / 8] |= 1 << (cpu % 8);
    }
    // SAFETY: pid 0 is the calling thread; the mask is a live buffer of the given size.
    let result = unsafe { sched_setaffinity(0, SET_BYTES, mask.as_ptr()) };
    if result == 0 {
        Ok(())
    } else {
        Err(format!(
            "sched_setaffinity {cpus:?}: {}",
            std::io::Error::last_os_error()
        ))
    }
}
