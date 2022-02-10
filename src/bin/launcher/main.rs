use std::{env, fs, mem, os::windows::prelude::FromRawHandle, ptr};

use anyhow::{anyhow, Context, Result};
use detour::Function;
use dll_syringe::{Process, ProcessModule, Syringe};
use steamlocate::SteamDir;

use windows::Win32::{
    Foundation::{CloseHandle, BOOL, HANDLE, HINSTANCE, MAX_PATH, PSTR, PWSTR},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, TH32CS_SNAPMODULE,
        },
        LibraryLoader::{FreeLibrary, GetProcAddress, LoadLibraryExW, DONT_RESOLVE_DLL_REFERENCES},
        ProcessStatus::{K32EnumProcesses, K32GetModuleFileNameExW},
        Threading::{
            CreateProcessA, CreateRemoteThread, OpenProcess, TerminateProcess, WaitForSingleObject,
            PROCESS_ALL_ACCESS, PROCESS_INFORMATION, PROCESS_QUERY_INFORMATION, STARTUPINFOA,
        },
    },
};

fn string_from_win32(buffer: &[u16]) -> String {
    let length = buffer
        .iter()
        .position(|c| *c == 0)
        .unwrap_or(buffer.len() + 1);
    String::from_utf16_lossy(&buffer[0..length])
}

struct ProcessInfo {
    pub process: HANDLE,
    pub process_id: u32,
}

fn unload_module(process: HANDLE, module: HINSTANCE) -> Result<()> {
    const PROC_NAME: PSTR = PSTR(b"unload\0".as_ptr() as _);

    unsafe {
        let mut buffer = [0_u16; MAX_PATH as _];
        let filename = PWSTR(buffer.as_mut_ptr());

        if K32GetModuleFileNameExW(process, module, filename, MAX_PATH) == 0 {
            return Err(anyhow!("failed to get module filename"));
        }

        let hmodule = LoadLibraryExW(filename, None, DONT_RESOLVE_DLL_REFERENCES);

        if hmodule.is_invalid() {
            return Err(anyhow!("failed to load module instance"));
        }

        let address = GetProcAddress(hmodule, PROC_NAME).context("failed to locate procedure");

        if !FreeLibrary(hmodule).as_bool() {
            return Err(anyhow!("failed to free module instance"));
        }

        let thread = CreateRemoteThread(
            process,
            ptr::null(),
            0,
            mem::transmute((address?.to_ptr() as isize - hmodule.0) + module.0),
            ptr::null(),
            0,
            ptr::null_mut(),
        );

        if thread.is_invalid() {
            return Err(anyhow!("failed to spawn remote thread"));
        }

        WaitForSingleObject(thread, u32::MAX);
        CloseHandle(thread);
    }

    Ok(())
}

fn load_module(process: HANDLE, module: HINSTANCE) -> Result<()> {
    const PROC_NAME: PSTR = PSTR(b"load\0".as_ptr() as _);

    unsafe {
        let mut buffer = [0_u16; MAX_PATH as _];
        let filename = PWSTR(buffer.as_mut_ptr());

        if K32GetModuleFileNameExW(process, module, filename, MAX_PATH) == 0 {
            return Err(anyhow!("failed to get module filename"));
        }

        let hmodule = LoadLibraryExW(filename, None, DONT_RESOLVE_DLL_REFERENCES);

        if hmodule.is_invalid() {
            return Err(anyhow!("failed to load module instance"));
        }

        let address = GetProcAddress(hmodule, PROC_NAME).context("failed to locate procedure");

        if !FreeLibrary(hmodule).as_bool() {
            return Err(anyhow!("failed to free module instance"));
        }

        let thread = CreateRemoteThread(
            process,
            ptr::null(),
            0,
            mem::transmute((address?.to_ptr() as isize - hmodule.0) + module.0),
            ptr::null(),
            0,
            ptr::null_mut(),
        );

        if thread.is_invalid() {
            return Err(anyhow!("failed to spawn remote thread"));
        }

        WaitForSingleObject(thread, u32::MAX);
        CloseHandle(thread);
    }

    Ok(())
}

const PAYLOAD_NAME: &str = "payload.dll";
const INJECTED_PAYLOAD_NAME: &str = "payload_loaded.dll";

fn find_module_handle(process_id: u32) -> Result<HINSTANCE> {
    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, process_id) };

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

    let modules = from_snapshot(handle, Module32FirstW, Module32NextW);

    Ok(modules
        .iter()
        .find(|module| {
            let module_name = string_from_win32(&module.szModule);
            module_name == INJECTED_PAYLOAD_NAME
        })
        .context("failed to find payload")?
        .hModule)
}

fn inject(process_info: &ProcessInfo) -> Result<()> {
    let process_id = process_info.process_id;
    let process_handle = process_info.process;
    let process = unsafe { Process::from_raw_handle(process_handle.0 as _) };

    let payload_path = env::current_exe()?.parent().unwrap().join(PAYLOAD_NAME);
    let injected_payload_path = env::current_exe()?
        .parent()
        .unwrap()
        .join(INJECTED_PAYLOAD_NAME);
    let terminate = |message| unsafe {
        TerminateProcess(process_info.process, 0);
        CloseHandle(process_info.process);
        Err(anyhow!("{message}"))
    };
    let syringe = Syringe::new();

    if let Ok(module_handle) = find_module_handle(process_id) {
        if let Err(error) = unload_module(process_handle, module_handle) {
            return terminate(error.to_string().as_str());
        }
        if let Err(error) = syringe
            .eject(unsafe { ProcessModule::new(mem::transmute(module_handle), process.get_ref()) })
        {
            return terminate(error.to_string().as_str());
        }
    }

    fs::copy(payload_path, injected_payload_path.clone()).context("failed to copy payload")?;

    if let Ok(module) = syringe.inject(&process, injected_payload_path) {
        let module_handle = unsafe { mem::transmute(module.handle()) };

        if let Err(error) = load_module(process_handle, module_handle) {
            return terminate(error.to_string().as_str());
        }
    } else {
        return terminate("failed to inject payload");
    }

    Ok(())
}

const PROCESS_NAME: &str = "HITMAN3.exe";

fn find_process() -> Result<ProcessInfo> {
    let mut process_ids = [0_u32; 1024];
    let process_count = unsafe {
        let size = mem::size_of_val(&process_ids) as u32;
        let mut returned_size = 0_u32;
        if !K32EnumProcesses(process_ids.as_mut_ptr(), size, &mut returned_size).as_bool() {
            return Err(anyhow!("failed to enumerate process"));
        }
        returned_size as usize / mem::size_of::<u32>()
    };

    process_ids
        .iter()
        .take(process_count)
        .find(|process_id| unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION, false, **process_id);
            if handle.is_invalid() {
                return false;
            }

            let mut buffer = [0_u16; MAX_PATH as _];
            let success = K32GetModuleFileNameExW(
                handle,
                HINSTANCE::default(),
                PWSTR(buffer.as_mut_ptr()),
                MAX_PATH,
            ) != 0;
            CloseHandle(handle);

            if !success {
                return false;
            }

            let filename = string_from_win32(&buffer);
            filename.ends_with(PROCESS_NAME)
        })
        .map(|process_id| unsafe {
            let process = OpenProcess(PROCESS_ALL_ACCESS, false, *process_id);

            if process.is_invalid() {
                return Err(anyhow!("failed to open process"));
            }

            Ok(ProcessInfo {
                process,
                process_id: *process_id,
            })
        })
        .context("failed to find process")?
}

const APP_ID: u32 = 1659040;

fn spawn_process() -> Result<ProcessInfo> {
    env::set_var("SteamAppId", APP_ID.to_string());

    let startup_info = STARTUPINFOA::default();
    let mut process_info = PROCESS_INFORMATION::default();
    let path = SteamDir::locate()
        .and_then(|mut x| x.app(&APP_ID).cloned())
        .map(|x| {
            x.path
                .join(format!("Retail/{PROCESS_NAME}\0"))
                .into_os_string()
                .into_string()
        })
        .and_then(|x| x.ok())
        .context("failed to locate install directory")?;

    unsafe {
        if !CreateProcessA(
            PSTR(path.as_ptr() as _),
            None,
            ptr::null(),
            ptr::null(),
            false,
            0,
            ptr::null(),
            None,
            &startup_info,
            &mut process_info,
        )
        .as_bool()
        {
            return Err(anyhow!("failed to spawn process"));
        }
    }

    Ok(ProcessInfo {
        process: process_info.hProcess,
        process_id: process_info.dwProcessId,
    })
}

fn main() -> Result<()> {
    if let Ok(process_info) = find_process() {
        inject(&process_info)?;
    } else {
        inject(&spawn_process()?)?;
    }

    Ok(())
}
