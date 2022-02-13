use std::{env, fs, os::windows::prelude::FromRawHandle, ptr};

use anyhow::{anyhow, Context, Result};

use dll_syringe::{Process, ProcessModule, ProcessRef, Syringe};
use steamlocate::SteamDir;

use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, PSTR},
    System::Threading::{CreateProcessA, TerminateProcess, PROCESS_INFORMATION, STARTUPINFOA},
};

const PAYLOAD_NAME: &str = "payload.dll";
const INJECTED_PAYLOAD_NAME: &str = "payload_loaded.dll";

fn unload_module(syringe: &mut Syringe, process_ref: ProcessRef) -> Result<()> {
    if let Some(process_module) = ProcessModule::find(INJECTED_PAYLOAD_NAME, process_ref)? {
        if let Some(mut unload_procedure) = syringe.get_procedure(process_module, "unload")? {
            let _: u64 = unload_procedure.call(&0_u64)?;
            println!("Unload procedure executed!");
        } else {
            return Err(anyhow!("failed to call unload procedure"));
        }

        syringe.eject(process_module)?;
    }

    Ok(())
}

fn load_module(syringe: &mut Syringe) -> Result<()> {
    let payload_path = env::current_exe()?.parent().unwrap().join(PAYLOAD_NAME);
    let injected_payload_path = env::current_exe()?
        .parent()
        .unwrap()
        .join(INJECTED_PAYLOAD_NAME);
    fs::copy(payload_path, injected_payload_path.clone()).context("failed to copy payload")?;

    let process_module = syringe.inject(injected_payload_path)?;

    if let Some(mut load_procedure) = syringe.get_procedure(process_module, "load")? {
        let _: u64 = load_procedure.call(&0_u64)?;
        println!("Load procedure executed!");
    } else {
        return Err(anyhow!("failed to call load procedure"));
    }

    Ok(())
}

fn inject(process: Process) -> Result<()> {
    let mut syringe = Syringe::for_process(&process);
    let terminate = |message| unsafe {
        let handle = HANDLE(process.handle() as _);
        TerminateProcess(handle, 0);
        CloseHandle(handle);
        Err(anyhow!("{message}"))
    };

    if let Err(error) = unload_module(&mut syringe, process.get_ref()) {
        return terminate(error.to_string().as_str());
    }

    if let Err(error) = load_module(&mut syringe) {
        return terminate(error.to_string().as_str());
    }

    Ok(())
}

const APP_ID: u32 = 1659040;
const PROCESS_NAME: &str = "HITMAN3.exe";

fn spawn_process() -> Result<Process> {
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

    Ok(unsafe { Process::from_raw_handle(process_info.hProcess.0 as _) })
}

fn main() -> Result<()> {
    if let Some(process) = Process::find_first_by_name(PROCESS_NAME).or(Some(spawn_process()?)) {
        println!("Attempting to inject...");
        inject(process)?;
    }

    Ok(())
}
