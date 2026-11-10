//! Thread quality-of-service control.
//!
//! Apple silicon does not support hard thread affinity
//! (`THREAD_AFFINITY_POLICY` returns `KERN_NOT_SUPPORTED`). QoS classes are the
//! supported lever: `UserInteractive` is preferred on performance cores and
//! `Background` is confined to efficiency cores.

/// QoS classes, values from `<sys/qos.h>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Qos {
    UserInteractive = 0x21,
    UserInitiated = 0x19,
    Default = 0x15,
    Utility = 0x11,
    Background = 0x09,
}

#[cfg(target_os = "macos")]
extern "C" {
    fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
}

/// Set the calling thread's QoS class. Returns the error code on failure.
#[cfg(target_os = "macos")]
pub fn set_current_thread(q: Qos) -> Result<(), i32> {
    let rc = unsafe { pthread_set_qos_class_self_np(q as u32, 0) };
    if rc == 0 { Ok(()) } else { Err(rc) }
}

/// Non-macOS: not supported.
#[cfg(not(target_os = "macos"))]
pub fn set_current_thread(_q: Qos) -> Result<(), i32> {
    Err(libc::ENOTSUP)
}
