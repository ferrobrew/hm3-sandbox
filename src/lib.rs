mod detouring;
mod game;
mod rendering;

use std::process;

use crate::detouring::prelude::*;
use c_string::c_str;
use windows::Win32::{
    Foundation::{HWND, PSTR},
    System::{Threading::{OpenThread, ResumeThread, THREAD_ALL_ACCESS}, Console::AllocConsole},
    UI::WindowsAndMessaging::MessageBoxA,
};

#[detour {
    name = "ZRenderer::Init",
    pattern = "48 89 4C 24 ? 55 53 56 57 41 54 41 55 41 57 48 8D AC 24 ? ? ? ? B8 ? ? ? ?"
}]
fn initialize_dx12(a1: u64) -> u64 {
    unsafe {
        MessageBoxA(
            HWND::default(),
            PSTR(c_str!("DX12 INTIALIZED").as_ptr() as _),
            PSTR(c_str!("ALERT").as_ptr() as _),
            0,
        );
        INITIALIZE_DX12.call(a1)
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "system" fn initialize(thread_id: usize) {
    let hthread = OpenThread(THREAD_ALL_ACCESS, false, thread_id as _);

    if hthread.is_invalid() {
        process::exit(0);
    }

    use libc::{fdopen, freopen};

    AllocConsole();

    let stdout = fdopen(1, c_str!("w").as_ptr());
    let stderr = fdopen(2, c_str!("w").as_ptr());
    freopen(c_str!("CONOUT$").as_ptr(), c_str!("w").as_ptr(), stdout);
    freopen(c_str!("CONOUT$").as_ptr(), c_str!("w").as_ptr(), stderr);

    let mut modules = Module::get_all();
    let module = modules
        .iter_mut()
        .find(|x| x.filename().as_deref() == Some("HITMAN3.exe"))
        .expect("failed to find module");

    rendering::hook(module).unwrap();
    //game::zapplication_engine_win32::hook(module).unwrap();
    //INITIALIZE_DX12_BINDER.bind(module).unwrap();
    //INITIALIZE_DX12_BINDER.enable().unwrap();

    ResumeThread(hthread);
}
