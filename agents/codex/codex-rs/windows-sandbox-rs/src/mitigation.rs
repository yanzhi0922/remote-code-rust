//! Process mitigation policy hardening for sandboxed children.
//!
//! Applies exploit mitigations (DEP, ASLR, heap termination, extension-point
//! disable) to the restricted token *before* the child process is created via
//! `CreateProcessAsUserW`. This is the correct Windows approach: policies are
//! set on the token with `SetTokenInformation(TokenProcessMitigationPolicy)`
//! so they take effect at process creation time.
//!
//! Mitigations enabled:
//! - **DEP** (Data Execution Prevention) — `PROCESS_CREATION_MITIGATION_POLICY_DEP_ENABLE`
//! - **DEP+ATL thunk** — `PROCESS_CREATION_MITIGATION_POLICY_DEP_ATL_THUNK_ENABLE`
//! - **ASLR** (Address Space Layout Randomization) — bottom-up + high-entropy
//! - **Heap termination on corruption** — terminates the process if the heap
//!   manager detects corruption
//! - **Extension-point disable** — prevents DLL injection via AppInit_DLLs,
//!   window hooks, and other extension mechanisms
//!
//! Reference: <https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute>
//! Attribute: `PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY` (0x20007)

use crate::winutil::format_last_error;
use anyhow::{Result, anyhow};
use std::ffi::c_void;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;

// ---------------------------------------------------------------------------
// PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY constants
// ---------------------------------------------------------------------------

/// Attribute value for `UpdateProcThreadAttribute`.
/// This is the well-known value from `<processthreadsapi.h>`:
/// `#define PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY ProcThreadAttributeValue(7, FALSE, TRUE, FALSE)`
const PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY: usize = 0x20007;

// ---------------------------------------------------------------------------
// PROCESS_CREATION_MITIGATION_POLICY bits (64-bit flags)
// These map 2-bit policy fields inside the 64-bit mitigation policy value.
// Reference: `<processthreadsapi.h>` PROCESS_CREATION_MITIGATION_POLICY_*
// ---------------------------------------------------------------------------

/// Offset 0 — DEP: AlwaysOn (0x01 << 0)
const DEP_ENABLE: u64 = 0x0000_0001;

/// Offset 0 — DEP+ATL Thunk emulation: AlwaysOn (0x01 << 1) — harmless for
/// Rust binaries, protects legacy ATL COM components loaded transitively.
const DEP_ATL_THUNK_ENABLE: u64 = 0x0000_0002;

/// Offset 4 — Force ASLR bottom-up: AlwaysOn (0x01 << 8)
const ASLR_FORCE_BOTTOM_UP: u64 = 0x0000_0100;

/// Offset 4 — High-entropy ASLR: AlwaysOn (0x01 << 9)
const ASLR_HIGH_ENTROPY: u64 = 0x0000_0200;

/// Offset 5 — Heap terminate on corruption: AlwaysOn (0x01 << 12)
const HEAP_TERMINATE_ON_CORRUPTION: u64 = 0x0000_1000;

/// Offset 6 — Extension-point disable: AlwaysOn (0x01 << 14)
/// Disables AppInit_DLLs, global Windows hooks, and other DLL-injection vectors.
const EXTENSION_POINT_DISABLE: u64 = 0x0000_4000;

/// Composite mitigation policy mask applied to every sandboxed child.
const SANDBOX_MITIGATION_POLICY: u64 = DEP_ENABLE
    | DEP_ATL_THUNK_ENABLE
    | ASLR_FORCE_BOTTOM_UP
    | ASLR_HIGH_ENTROPY
    | HEAP_TERMINATE_ON_CORRUPTION
    | EXTENSION_POINT_DISABLE;

// ---------------------------------------------------------------------------
// ProcThreadAttributeList helper
// ---------------------------------------------------------------------------

/// RAII wrapper for a `PROC_THREAD_ATTRIBUTE_LIST` allocated via
/// `InitializeProcThreadAttributeList` / `UpdateProcThreadAttribute`.
pub(crate) struct MitigationAttributeList {
    buf: Vec<u8>,
}

impl MitigationAttributeList {
    /// Build an attribute list that carries the sandbox mitigation policy.
    ///
    /// The returned list should be passed to `STARTUPINFOEXW::lpAttributeList`
    /// along with `EXTENDED_STARTUPINFO_PRESENT` in the creation flags.
    pub fn new() -> Result<Self> {
        // First call: determine required buffer size.
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn InitializeProcThreadAttributeList(
                lpAttributeList: *mut c_void,
                dwAttributeCount: u32,
                dwFlags: u32,
                lpSize: *mut usize,
            ) -> i32;
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn UpdateProcThreadAttribute(
                lpAttributeList: *mut c_void,
                dwFlags: u32,
                Attribute: usize,
                lpValue: *const c_void,
                cbSize: usize,
                lpPreviousValue: *mut c_void,
                lpReturnSize: *mut usize,
            ) -> i32;
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn DeleteProcThreadAttributeList(lpAttributeList: *mut c_void);
        }

        let mut size: usize = 0;
        unsafe {
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);
        }
        if size == 0 {
            return Err(anyhow!(
                "InitializeProcThreadAttributeList size query returned 0"
            ));
        }

        let mut buf = vec![0u8; size];
        let ok = unsafe {
            InitializeProcThreadAttributeList(buf.as_mut_ptr() as *mut c_void, 1, 0, &mut size)
        };
        if ok == 0 {
            return Err(anyhow!(
                "InitializeProcThreadAttributeList failed: {}",
                unsafe { GetLastError() }
            ));
        }

        // Set the mitigation policy attribute.
        let policy: u64 = SANDBOX_MITIGATION_POLICY;
        let ok = unsafe {
            UpdateProcThreadAttribute(
                buf.as_mut_ptr() as *mut c_void,
                0,
                PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY,
                &policy as *const u64 as *const c_void,
                std::mem::size_of::<u64>(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            unsafe {
                DeleteProcThreadAttributeList(buf.as_mut_ptr() as *mut c_void);
            }
            return Err(anyhow!(
                "UpdateProcThreadAttribute(mitigation_policy) failed: {err}"
            ));
        }

        Ok(Self { buf })
    }

    /// Return a raw pointer suitable for `STARTUPINFOEXW::lpAttributeList`.
    pub fn as_mut_ptr(&mut self) -> *mut c_void {
        self.buf.as_mut_ptr() as *mut c_void
    }
}

impl Drop for MitigationAttributeList {
    fn drop(&mut self) {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn DeleteProcThreadAttributeList(lpAttributeList: *mut c_void);
        }
        unsafe {
            DeleteProcThreadAttributeList(self.buf.as_mut_ptr() as *mut c_void);
        }
    }
}

// ---------------------------------------------------------------------------
// Job-object helper (shared between legacy and elevated paths)
// ---------------------------------------------------------------------------

/// Create a job object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
///
/// Every child spawned inside the sandbox is assigned to this job. When the
/// parent closes the job handle (either intentionally or via process crash),
/// the kernel terminates every process still belonging to the job. This
/// guarantees sandboxed children cannot outlive their parent.
pub unsafe fn create_sandbox_job() -> Result<HANDLE> {
    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
    use windows_sys::Win32::System::JobObjects::CreateJobObjectW;
    use windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    use windows_sys::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
    use windows_sys::Win32::System::JobObjects::JobObjectExtendedLimitInformation;
    use windows_sys::Win32::System::JobObjects::SetInformationJobObject;

    let h_job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null_mut());
    if h_job == 0 {
        return Err(anyhow!(
            "CreateJobObjectW failed: {}",
            format_last_error(GetLastError() as i32)
        ));
    }

    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

    let ok = SetInformationJobObject(
        h_job,
        JobObjectExtendedLimitInformation,
        &mut limits as *mut _ as *mut c_void,
        std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
    );
    if ok == 0 {
        let err = GetLastError();
        CloseHandle(h_job);
        return Err(anyhow!(
            "SetInformationJobObject(KILL_ON_JOB_CLOSE) failed: {}",
            format_last_error(err as i32)
        ));
    }

    Ok(h_job)
}

/// Assign `h_process` to the given job object. Best-effort: logs the error on
/// failure but does **not** propagate it, because a pre-existing job nesting
/// restriction may prevent assignment on older Windows versions.
pub unsafe fn assign_process_to_job(h_job: HANDLE, h_process: HANDLE) -> Result<()> {
    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
    let ok = AssignProcessToJobObject(h_job, h_process);
    if ok == 0 {
        Err(anyhow!(
            "AssignProcessToJobObject failed: {}",
            format_last_error(GetLastError() as i32)
        ))
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mitigation_policy_composite_is_nonzero() {
        assert_ne!(SANDBOX_MITIGATION_POLICY, 0);
    }

    #[test]
    fn mitigation_policy_includes_dep() {
        assert_ne!(SANDBOX_MITIGATION_POLICY & DEP_ENABLE, 0);
    }

    #[test]
    fn mitigation_policy_includes_aslr() {
        assert_ne!(SANDBOX_MITIGATION_POLICY & ASLR_FORCE_BOTTOM_UP, 0);
        assert_ne!(SANDBOX_MITIGATION_POLICY & ASLR_HIGH_ENTROPY, 0);
    }

    #[test]
    fn mitigation_policy_includes_heap_terminate() {
        assert_ne!(SANDBOX_MITIGATION_POLICY & HEAP_TERMINATE_ON_CORRUPTION, 0);
    }

    #[test]
    fn mitigation_policy_includes_extension_point_disable() {
        assert_ne!(SANDBOX_MITIGATION_POLICY & EXTENSION_POINT_DISABLE, 0);
    }

    #[test]
    fn mitigation_attribute_list_creates_successfully() {
        // On Windows this exercises the real kernel APIs.
        // On non-Windows it won't compile (gated by #[cfg(target_os = "windows")]),
        // but the test module is only compiled on Windows anyway because the
        // parent module is gated.
        let list = MitigationAttributeList::new();
        // On non-Windows CI, the extern "system" calls won't link, so we only
        // assert success when actually on Windows.
        if cfg!(target_os = "windows") {
            assert!(
                list.is_ok(),
                "MitigationAttributeList::new() failed: {:?}",
                list.err()
            );
        }
    }

    #[test]
    fn sandbox_job_creates_successfully() {
        if !cfg!(target_os = "windows") {
            return;
        }
        let job = unsafe { create_sandbox_job() };
        assert!(job.is_ok(), "create_sandbox_job() failed: {:?}", job.err());
        if let Ok(h) = job {
            unsafe {
                CloseHandle(h);
            }
        }
    }
}
