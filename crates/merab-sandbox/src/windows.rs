use anyhow::{Context, Result};
use std::mem;
use tracing::{debug, warn};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

pub struct JobObject {
    handle: HANDLE,
}

impl JobObject {
    pub fn new() -> Result<Self> {
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error()).context("Failed to create job object");
            }

            let mut job = Self { handle };
            if let Err(e) = job.set_kill_on_close() {
                warn!("Failed to set kill-on-close for job object: {:?}", e);
                return Err(e);
            }
            Ok(job)
        }
    }

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

    pub fn assign_process(&self, pid: u32) -> Result<()> {
        unsafe {
            let process_handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if process_handle.is_null() {
                return Err(std::io::Error::last_os_error())
                    .with_context(|| format!("Failed to open process {}", pid));
            }

            let result = AssignProcessToJobObject(self.handle, process_handle);
            let assign_err = if result == 0 {
                Some(std::io::Error::last_os_error())
            } else {
                None
            };

            CloseHandle(process_handle);

            if let Some(err) = assign_err {
                return Err(err).context("Failed to assign process to job object");
            }

            debug!("Assigned process {} to job object", pid);
            Ok(())
        }
    }

    pub fn set_memory_limit(&self, limit_bytes: usize) -> Result<()> {
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
        // We MUST preserve existing flags (like kill-on-close)
        info.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
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

    pub fn get_memory_usage(&self) -> Result<usize> {
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };

        let result = unsafe {
            QueryInformationJobObject(
                self.handle,
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as *mut _,
                mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                std::ptr::null_mut(),
            )
        };

        if result == 0 {
            return Err(std::io::Error::last_os_error()).context("Failed to query job information");
        }

        // Return peak memory usage as a proxy for current pressure
        Ok(info.PeakProcessMemoryUsed)
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

unsafe impl Send for JobObject {}
unsafe impl Sync for JobObject {}
