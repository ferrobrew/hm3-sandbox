mod detouring;
mod game;
mod rendering;

use std::process;

use crate::detouring::prelude::*;
use c_string::c_str;
use windows::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_ALL_ACCESS};

#[cfg(feature = "debug-console")]
fn alloc_console() {
    use libc::{fdopen, freopen};
    use windows::Win32::System::Console::AllocConsole;

    unsafe {
        AllocConsole();
        let stdout = fdopen(1, c_str!("w").as_ptr());
        let stderr = fdopen(2, c_str!("w").as_ptr());
        freopen(c_str!("CONOUT$").as_ptr(), c_str!("w").as_ptr(), stdout);
        freopen(c_str!("CONOUT$").as_ptr(), c_str!("w").as_ptr(), stderr);
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "system" fn initialize(thread_id: usize) {
    let hthread = OpenThread(THREAD_ALL_ACCESS, false, thread_id as _);

    if hthread.is_invalid() {
        process::exit(0);
    }

    #[cfg(feature = "debug-console")]
    alloc_console();

    let mut modules = Module::get_all();
    let module = modules
        .iter_mut()
        .find(|x| x.filename().as_deref() == Some("HITMAN3.exe"))
        .expect("failed to find module");

    rendering::hook().unwrap();
    game::zrender::hook(module).unwrap();
    game::zapplication_engine_win32::hook(module).unwrap();

    ResumeThread(hthread);
}
