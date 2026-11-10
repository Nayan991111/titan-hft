//! Hardware performance counters on Apple silicon through `kperf.framework`
//! and `kperfdata.framework`.
//!
//! Both are private frameworks living in the dyld shared cache, so they are
//! opened with `dlopen` by path (the files do not exist on disk) and every
//! symbol is resolved with `dlsym`. Event encodings come from the CPU's own
//! database in `/usr/share/kpep/`, selected automatically by
//! `kpep_db_create(NULL)`, so no raw event numbers are hard-coded.
//!
//! Requirements: root (`sudo`), and no other PMU client running
//! (`powermetrics`, Instruments), otherwise the counters are busy.
//!
//! `kpc_get_thread_counters` enters the kernel. Its cost is far larger than
//! the operations studied here, so it is used to bracket batches of
//! iterations, never single events. [`Session::read_overhead`] measures it.
//!
//! The function signatures follow the reverse-engineered declarations
//! published in ibireme's `kpc_demo.c`.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::fmt;

const KPERF_PATH: &str = "/System/Library/PrivateFrameworks/kperf.framework/kperf";
const KPERFDATA_PATH: &str = "/System/Library/PrivateFrameworks/kperfdata.framework/kperfdata";

/// Upper bound on counters returned by the kernel.
pub const MAX_COUNTERS: usize = 32;

const KPC_CLASS_CONFIGURABLE_MASK: u32 = 1 << 1;

#[derive(Debug)]
pub enum KperfError {
    /// A framework could not be opened (not macOS, or path changed).
    Load(String),
    /// A symbol is missing from the framework.
    Symbol(&'static str),
    /// A call returned a non-zero status.
    Call(&'static str, c_int),
    /// None of the candidate names exist in this CPU's event database.
    EventNotFound(Vec<String>),
}

impl fmt::Display for KperfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KperfError::Load(p) => write!(f, "cannot dlopen {p}"),
            KperfError::Symbol(s) => write!(f, "missing symbol {s}"),
            KperfError::Call(s, rc) => write!(f, "{s} failed with status {rc} (root required; is powermetrics or Instruments running?)"),
            KperfError::EventNotFound(c) => write!(f, "no event matches any of {c:?}"),
        }
    }
}

impl std::error::Error for KperfError {}

type Db = c_void;
type Config = c_void;
type Event = c_void;

#[allow(non_snake_case)]
struct Api {
    kpc_pmu_version: unsafe extern "C" fn() -> u32,
    kpc_cpu_string: unsafe extern "C" fn(*mut c_char, usize) -> c_int,
    kpc_set_counting: unsafe extern "C" fn(u32) -> c_int,
    kpc_set_thread_counting: unsafe extern "C" fn(u32) -> c_int,
    kpc_set_config: unsafe extern "C" fn(u32, *mut u64) -> c_int,
    kpc_get_thread_counters: unsafe extern "C" fn(u32, u32, *mut u64) -> c_int,
    kpc_force_all_ctrs_set: unsafe extern "C" fn(c_int) -> c_int,
    kpc_force_all_ctrs_get: unsafe extern "C" fn(*mut c_int) -> c_int,

    kpep_db_create: unsafe extern "C" fn(*const c_char, *mut *mut Db) -> c_int,
    kpep_db_free: unsafe extern "C" fn(*mut Db),
    kpep_db_name: unsafe extern "C" fn(*mut Db, *mut *const c_char) -> c_int,
    kpep_db_events_count: unsafe extern "C" fn(*mut Db, *mut usize) -> c_int,
    kpep_db_events: unsafe extern "C" fn(*mut Db, *mut *mut Event, usize) -> c_int,
    kpep_db_event: unsafe extern "C" fn(*mut Db, *const c_char, *mut *mut Event) -> c_int,
    kpep_event_name: unsafe extern "C" fn(*mut Event, *mut *const c_char) -> c_int,
    kpep_event_alias: unsafe extern "C" fn(*mut Event, *mut *const c_char) -> c_int,
    kpep_event_description: unsafe extern "C" fn(*mut Event, *mut *const c_char) -> c_int,
    kpep_config_create: unsafe extern "C" fn(*mut Db, *mut *mut Config) -> c_int,
    kpep_config_free: unsafe extern "C" fn(*mut Config),
    kpep_config_force_counters: unsafe extern "C" fn(*mut Config) -> c_int,
    kpep_config_add_event: unsafe extern "C" fn(*mut Config, *mut *mut Event, u32, *mut u32) -> c_int,
    kpep_config_kpc_classes: unsafe extern "C" fn(*mut Config, *mut u32) -> c_int,
    kpep_config_kpc_count: unsafe extern "C" fn(*mut Config, *mut usize) -> c_int,
    kpep_config_kpc_map: unsafe extern "C" fn(*mut Config, *mut usize, usize) -> c_int,
    kpep_config_kpc: unsafe extern "C" fn(*mut Config, *mut u64, usize) -> c_int,
}

fn open(path: &str) -> Result<*mut c_void, KperfError> {
    let c = CString::new(path).expect("path has no NUL");
    let h = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_LAZY) };
    if h.is_null() {
        Err(KperfError::Load(path.to_string()))
    } else {
        Ok(h)
    }
}

macro_rules! sym {
    ($h:expr, $name:literal) => {{
        let p = unsafe { libc::dlsym($h, concat!($name, "\0").as_ptr() as *const c_char) };
        if p.is_null() {
            return Err(KperfError::Symbol($name));
        }
        unsafe { std::mem::transmute::<*mut c_void, _>(p) }
    }};
}

impl Api {
    fn load() -> Result<Api, KperfError> {
        let k = open(KPERF_PATH)?;
        let d = open(KPERFDATA_PATH)?;
        Ok(Api {
            kpc_pmu_version: sym!(k, "kpc_pmu_version"),
            kpc_cpu_string: sym!(k, "kpc_cpu_string"),
            kpc_set_counting: sym!(k, "kpc_set_counting"),
            kpc_set_thread_counting: sym!(k, "kpc_set_thread_counting"),
            kpc_set_config: sym!(k, "kpc_set_config"),
            kpc_get_thread_counters: sym!(k, "kpc_get_thread_counters"),
            kpc_force_all_ctrs_set: sym!(k, "kpc_force_all_ctrs_set"),
            kpc_force_all_ctrs_get: sym!(k, "kpc_force_all_ctrs_get"),
            kpep_db_create: sym!(d, "kpep_db_create"),
            kpep_db_free: sym!(d, "kpep_db_free"),
            kpep_db_name: sym!(d, "kpep_db_name"),
            kpep_db_events_count: sym!(d, "kpep_db_events_count"),
            kpep_db_events: sym!(d, "kpep_db_events"),
            kpep_db_event: sym!(d, "kpep_db_event"),
            kpep_event_name: sym!(d, "kpep_event_name"),
            kpep_event_alias: sym!(d, "kpep_event_alias"),
            kpep_event_description: sym!(d, "kpep_event_description"),
            kpep_config_create: sym!(d, "kpep_config_create"),
            kpep_config_free: sym!(d, "kpep_config_free"),
            kpep_config_force_counters: sym!(d, "kpep_config_force_counters"),
            kpep_config_add_event: sym!(d, "kpep_config_add_event"),
            kpep_config_kpc_classes: sym!(d, "kpep_config_kpc_classes"),
            kpep_config_kpc_count: sym!(d, "kpep_config_kpc_count"),
            kpep_config_kpc_map: sym!(d, "kpep_config_kpc_map"),
            kpep_config_kpc: sym!(d, "kpep_config_kpc"),
        })
    }
}

fn check(name: &'static str, rc: c_int) -> Result<(), KperfError> {
    if rc == 0 { Ok(()) } else { Err(KperfError::Call(name, rc)) }
}

unsafe fn cstr(p: *const c_char) -> String {
    if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
}

/// One entry of the CPU's event database.
#[derive(Clone, Debug)]
pub struct EventInfo {
    pub name: String,
    pub alias: String,
    pub description: String,
}

/// The loaded frameworks plus this CPU's event database.
pub struct Kperf {
    api: Api,
    db: *mut Db,
}

impl Kperf {
    /// Load the frameworks and open the event database for the running CPU.
    /// Works without root; configuring counters needs root.
    pub fn open() -> Result<Kperf, KperfError> {
        let api = Api::load()?;
        let mut db = std::ptr::null_mut();
        check("kpep_db_create", unsafe { (api.kpep_db_create)(std::ptr::null(), &mut db) })?;
        Ok(Kperf { api, db })
    }

    pub fn database_name(&self) -> String {
        let mut p = std::ptr::null();
        unsafe {
            (self.api.kpep_db_name)(self.db, &mut p);
            cstr(p)
        }
    }

    pub fn pmu_version(&self) -> u32 {
        unsafe { (self.api.kpc_pmu_version)() }
    }

    pub fn cpu_string(&self) -> String {
        let mut buf = [0 as c_char; 128];
        unsafe {
            if (self.api.kpc_cpu_string)(buf.as_mut_ptr(), buf.len()) != 0 {
                return String::new();
            }
            cstr(buf.as_ptr())
        }
    }

    /// True if the process may program counters (normally means running as root).
    pub fn has_permission(&self) -> bool {
        let mut v: c_int = 0;
        unsafe { (self.api.kpc_force_all_ctrs_get)(&mut v) == 0 }
    }

    /// Every event in the database, sorted by name.
    pub fn events(&self) -> Result<Vec<EventInfo>, KperfError> {
        let mut n = 0usize;
        check("kpep_db_events_count", unsafe { (self.api.kpep_db_events_count)(self.db, &mut n) })?;
        let mut evs: Vec<*mut Event> = vec![std::ptr::null_mut(); n];
        check("kpep_db_events", unsafe {
            (self.api.kpep_db_events)(self.db, evs.as_mut_ptr(), n * std::mem::size_of::<*mut Event>())
        })?;
        let mut out: Vec<EventInfo> = evs
            .into_iter()
            .filter(|e| !e.is_null())
            .map(|e| unsafe { self.info(e) })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    unsafe fn info(&self, e: *mut Event) -> EventInfo {
        let (mut n, mut a, mut d) = (std::ptr::null(), std::ptr::null(), std::ptr::null());
        (self.api.kpep_event_name)(e, &mut n);
        (self.api.kpep_event_alias)(e, &mut a);
        (self.api.kpep_event_description)(e, &mut d);
        EventInfo { name: cstr(n), alias: cstr(a), description: cstr(d) }
    }

    fn lookup(&self, candidates: &[&str]) -> Result<(*mut Event, String), KperfError> {
        for &c in candidates {
            let name = CString::new(c).expect("event name has no NUL");
            let mut ev = std::ptr::null_mut();
            if unsafe { (self.api.kpep_db_event)(self.db, name.as_ptr(), &mut ev) } == 0 && !ev.is_null() {
                let resolved = unsafe { self.info(ev) }.name;
                return Ok((ev, if resolved.is_empty() { c.to_string() } else { resolved }));
            }
        }
        Err(KperfError::EventNotFound(candidates.iter().map(|s| s.to_string()).collect()))
    }

    /// Program the counters for the calling process. Each element of `events`
    /// is a list of candidate names (canonical name first, aliases after); the
    /// first one present in this CPU's database is used.
    pub fn session(self, events: &[&[&str]]) -> Result<Session, KperfError> {
        let api = &self.api;
        let mut cfg = std::ptr::null_mut();
        check("kpep_config_create", unsafe { (api.kpep_config_create)(self.db, &mut cfg) })?;
        let mut session = Session { kperf: self, cfg, classes: 0, map: Vec::new(), names: Vec::new(), running: false };
        let api = &session.kperf.api;
        check("kpep_config_force_counters", unsafe { (api.kpep_config_force_counters)(cfg) })?;
        for cands in events {
            let (mut ev, name) = session.kperf.lookup(cands)?;
            let mut err = 0u32;
            check("kpep_config_add_event", unsafe { (api.kpep_config_add_event)(cfg, &mut ev, 0, &mut err) })?;
            session.names.push(name);
        }
        let mut classes = 0u32;
        check("kpep_config_kpc_classes", unsafe { (api.kpep_config_kpc_classes)(cfg, &mut classes) })?;
        let mut reg_count = 0usize;
        check("kpep_config_kpc_count", unsafe { (api.kpep_config_kpc_count)(cfg, &mut reg_count) })?;
        let mut map = [0usize; MAX_COUNTERS];
        check("kpep_config_kpc_map", unsafe {
            (api.kpep_config_kpc_map)(cfg, map.as_mut_ptr(), std::mem::size_of_val(&map))
        })?;
        let mut regs = [0u64; MAX_COUNTERS];
        check("kpep_config_kpc", unsafe {
            (api.kpep_config_kpc)(cfg, regs.as_mut_ptr(), std::mem::size_of_val(&regs))
        })?;

        check("kpc_force_all_ctrs_set", unsafe { (api.kpc_force_all_ctrs_set)(1) })?;
        if classes & KPC_CLASS_CONFIGURABLE_MASK != 0 && reg_count > 0 {
            check("kpc_set_config", unsafe { (api.kpc_set_config)(classes, regs.as_mut_ptr()) })?;
        }
        check("kpc_set_counting", unsafe { (api.kpc_set_counting)(classes) })?;
        check("kpc_set_thread_counting", unsafe { (api.kpc_set_thread_counting)(classes) })?;
        session.classes = classes;
        session.map = map[..events.len()].to_vec();
        session.running = true;
        Ok(session)
    }
}

impl Drop for Kperf {
    fn drop(&mut self) {
        if !self.db.is_null() {
            unsafe { (self.api.kpep_db_free)(self.db) };
        }
    }
}

/// Raw counter snapshot for the calling thread.
#[derive(Clone, Copy)]
pub struct Snapshot(pub [u64; MAX_COUNTERS]);

/// Programmed counters. Dropping it stops counting and releases the PMU.
pub struct Session {
    kperf: Kperf,
    cfg: *mut Config,
    classes: u32,
    map: Vec<usize>,
    names: Vec<String>,
    running: bool,
}

impl Session {
    /// Resolved event names, in the order they were requested.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Read the calling thread's counters. Serialised with `dmb ish; isb` so
    /// earlier memory accesses and instructions complete before the read.
    #[inline(always)]
    pub fn read(&self) -> Snapshot {
        let mut buf = [0u64; MAX_COUNTERS];
        serialize();
        unsafe { (self.kperf.api.kpc_get_thread_counters)(0, MAX_COUNTERS as u32, buf.as_mut_ptr()) };
        serialize();
        Snapshot(buf)
    }

    /// Per-event deltas between two snapshots, in request order.
    pub fn delta(&self, start: &Snapshot, end: &Snapshot) -> Vec<u64> {
        self.map.iter().map(|&i| end.0[i].wrapping_sub(start.0[i])).collect()
    }

    /// Counter deltas of two back-to-back reads, `reps` times. Element `k` of
    /// each returned row is the overhead seen by event `k`.
    pub fn read_overhead(&self, reps: usize) -> Vec<Vec<u64>> {
        (0..reps)
            .map(|_| {
                let a = self.read();
                let b = self.read();
                self.delta(&a, &b)
            })
            .collect()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let api = &self.kperf.api;
        unsafe {
            if self.running {
                (api.kpc_set_thread_counting)(0);
                (api.kpc_set_counting)(0);
                (api.kpc_force_all_ctrs_set)(0);
            }
            if !self.cfg.is_null() {
                (api.kpep_config_free)(self.cfg);
            }
        }
        let _ = self.classes;
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn serialize() {
    unsafe { core::arch::asm!("dmb ish", "isb", options(nostack, preserves_flags)) };
}

#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
fn serialize() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
}

/// Candidate names for the events the paper uses. Canonical database names
/// first, then the generic aliases kperfdata also accepts.
pub mod events {
    pub const CYCLES: &[&str] = &["FIXED_CYCLES", "CORE_ACTIVE_CYCLE", "Cycles"];
    pub const INSTRUCTIONS: &[&str] = &["FIXED_INSTRUCTIONS", "INST_ALL", "Instructions"];
    pub const L1D_LOAD_MISS: &[&str] = &["L1D_CACHE_MISS_LD", "L1D_CACHE_MISS_LD_NONSPEC"];
    pub const BRANCH_MISPREDICT: &[&str] = &["BRANCH_MISPRED_NONSPEC", "Branch Mispredictions"];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn reports_unavailable_off_macos() {
        assert!(matches!(Kperf::open(), Err(KperfError::Load(_))));
    }
}
