use std::{env, mem, os::windows::prelude::FromRawHandle, ptr};

use anyhow::{anyhow, Context, Result};
use detour::Function;
use dll_syringe::{Process, Syringe};
use steamlocate::SteamDir;

use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HINSTANCE, MAX_PATH, PSTR, PWSTR},
    System::{
        LibraryLoader::{FreeLibrary, GetProcAddress, LoadLibraryExW, DONT_RESOLVE_DLL_REFERENCES},
        ProcessStatus::K32GetModuleFileNameExW,
        Threading::{
            CreateProcessA, CreateRemoteThread, TerminateProcess, CREATE_SUSPENDED,
            PROCESS_INFORMATION, STARTUPINFOA,
        },
    },
};

#[cfg(feature = "vscode-lldb")]
fn attach_debugger(pid: u32) -> Result<()> {
    use std::{thread, time::Duration};
    use windows::Win32::UI::Shell::ShellExecuteA;

    let operation = "open\0";
    let command = format!(
            "vscode-insiders://vadimcn.vscode-lldb/launch/config?{{'sourceLanguages':['rust'],'request':'attach','pid':{pid}}}\0"
        );

    unsafe {
        if ShellExecuteA(
            None,
            PSTR(operation.as_ptr() as _),
            PSTR(command.as_ptr() as _),
            None,
            None,
            0,
        )
        .is_invalid()
        {
            return Err(anyhow!("failed to attach debugger"));
        }
    }

    thread::sleep(Duration::from_millis(1000_u64));
    Ok(())
}

const PROC_NAME: PSTR = PSTR(b"initialize\0".as_ptr() as _);

fn initialize_module(process: HANDLE, module: HINSTANCE, thread_id: u32) -> Result<()> {
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
            thread_id as _,
            0,
            ptr::null_mut(),
        );

        if thread.is_invalid() {
            return Err(anyhow!("failed to spawn remote thread"));
        }

        CloseHandle(thread);
    }

    Ok(())
}

const APP_ID: u32 = 1659040;

fn main() -> Result<()> {
    env::set_var("SteamAppId", APP_ID.to_string());

    let path = SteamDir::locate()
        .and_then(|mut x| x.app(&APP_ID).cloned())
        .map(|x| {
            x.path
                .join("Retail/HITMAN3.exe\0")
                .into_os_string()
                .into_string()
        })
        .and_then(|x| x.ok())
        .context("failed to locate install directory")?;

    unsafe {
        let startup_info = STARTUPINFOA::default();
        let mut process_info = PROCESS_INFORMATION::default();

        if !CreateProcessA(
            PSTR(path.as_ptr() as _),
            None,
            ptr::null(),
            ptr::null(),
            false,
            CREATE_SUSPENDED,
            ptr::null(),
            None,
            &startup_info,
            &mut process_info,
        )
        .as_bool()
        {
            return Err(anyhow!("failed to spawn process"));
        }

        let terminate = |message| {
            TerminateProcess(process_info.hProcess, 0);
            CloseHandle(process_info.hProcess);
            CloseHandle(process_info.hThread);
            Err(anyhow!("{message}"))
        };

        #[cfg(feature = "vscode-lldb")]
        attach_debugger(process_info.dwProcessId)?;

        let process_handle = process_info.hProcess;
        let process = Process::from_raw_handle(process_handle.0 as _);
        let payload = env::current_exe()?.parent().unwrap().join("payload.dll");

        if let Ok(module) = Syringe::new().inject(&process, payload) {
            let module_handle = unsafe { mem::transmute(module.handle()) };

            if let Err(initialization_error) =
                initialize_module(process_handle, module_handle, process_info.dwThreadId)
            {
                return terminate(initialization_error.to_string().as_str());
            }
        } else {
            return terminate("failed to inject payload");
        }
    }

    Ok(())
}
