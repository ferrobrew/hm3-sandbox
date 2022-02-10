use std::mem;

use anyhow::{anyhow, Result};
use windows::Win32::{
    Foundation::{CloseHandle, BOOL, HANDLE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
        },
        Threading::{
            GetCurrentProcessId, GetCurrentThreadId, OpenThread, ResumeThread, SuspendThread,
            THREAD_ALL_ACCESS,
        },
    },
};

pub struct ThreadGroup {
    threads: Vec<HANDLE>,
}

impl ThreadGroup {
    pub fn new() -> Result<Self> {
        let process_id = unsafe { GetCurrentProcessId() };
        let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, process_id) };

        if handle.is_invalid() {
            return Err(anyhow!("failed to create snapshot of process"));
        }

        fn from_snapshot<Value: Default + Copy + Sized>(
            handle: HANDLE,
            first: unsafe fn(HANDLE, *mut Value) -> BOOL,
            next: unsafe fn(HANDLE, *mut Value) -> BOOL,
        ) -> Vec<Value> {
            unsafe {
                let mut value = Default::default();
                let size: *mut u32 = mem::transmute(&mut value);
                *size = mem::size_of::<Value>() as u32;
                let mut values = Vec::with_capacity(64);
                if first(handle, &mut value).as_bool() {
                    values.push(value);
                    while next(handle, &mut value).as_bool() {
                        values.push(value);
                    }
                }
                values
            }
        }

        let thread_id = unsafe { GetCurrentThreadId() };
        let threads = from_snapshot(handle, Thread32First, Thread32Next)
            .iter()
            .filter(|thread| {
                thread.th32OwnerProcessID == process_id && thread.th32ThreadID != thread_id
            })
            .map(|thread| unsafe { OpenThread(THREAD_ALL_ACCESS, false, thread.th32ThreadID) })
            .collect();

        Ok(Self { threads })
    }

    pub fn suspend(&self) {
        for handle in &self.threads {
            unsafe { SuspendThread(handle) };
        }
    }

    pub fn resume(&self) {
        for handle in &self.threads {
            unsafe { ResumeThread(handle) };
        }
    }

    pub fn release(&mut self) {
        for handle in &self.threads {
            unsafe { CloseHandle(handle) };
        }
        self.threads.clear();
    }
}

impl Drop for ThreadGroup {
    fn drop(&mut self) {
        self.release();
    }
}
