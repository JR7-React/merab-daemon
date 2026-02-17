use anyhow::{Context, Result};
use std::mem;
use std::os::windows::io::RawHandle;
use tracing::{debug, warn};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

pub struct JobObject {
    handle: HANDLE,
}

impl JobObject {
    /// Create a new Job Object for process isolation.
    /// By default, processes in this job will be terminated when the job handle is closed.
    pub fn new() -> Result<Self> {
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error()).context("Failed to create job object");
            }

            let mut job = Self { handle };
            if let Err(e) = job.set_kill_on_close() {
                // If we can't set limits, try to cleanup handle and return error
                // Drop will handle cleanup but let's be explicit
                warn!("Failed to set kill-on-close for job object: {:?}", e);
                return Err(e);
            }
            Ok(job)
        }
    }

    /// Configure the job to kill all child processes when the job handle is closed (e.g. daemon exit).
    fn set_kill_on_close(&mut self) -> Result<()> {
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let result = unsafe {
            SetInformationJobObject(
                self.handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };

        if result == 0 {
            return Err(std::io::Error::last_os_error()).context("Failed to set job limits");
        }
        Ok(())
    }

    /// Assign a running process (via PID) to this job object.
    pub fn assign_process(&self, pid: u32) -> Result<()> {
        unsafe {
            // We need PROCESS_SET_QUOTA and PROCESS_TERMINATE rights to assign to job
            let process_handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if process_handle.is_null() {
                return Err(std::io::Error::last_os_error())
                    .with_context(|| format!("Failed to open process {}", pid));
            }

            // Assign
            let result = AssignProcessToJobObject(self.handle, process_handle);
            let assign_err = if result == 0 {
                Some(std::io::Error::last_os_error())
            } else {
                None
            };

            // Close process handle (we only needed it for assignment)
            CloseHandle(process_handle);

            if let Some(err) = assign_err {
                return Err(err).context("Failed to assign process to job object");
            }
            
            debug!("Assigned process {} to job object", pid);
            Ok(())
        }
    }

    /// Set memory limit for processes in the job.
    pub fn set_memory_limit(&self, limit_bytes: usize) -> Result<()> {
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_PROCESS_MEMORY;
        info.ProcessMemoryLimit = limit_bytes;

        let result = unsafe {
            SetInformationJobObject(
                self.handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };

        if result == 0 {
            return Err(std::io::Error::last_os_error()).context("Failed to set job memory limit");
        }
        Ok(())
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

// Make sure it's Send/Sync as HANDLE is just a pointer-sized integer
unsafe impl Send for JobObject {}
unsafe impl Sync for JobObject {}
