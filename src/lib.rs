mod detouring;
mod game;
mod rendering;

use std::{mem, thread};

use crate::detouring::prelude::*;
use c_string::c_str;
use lazy_static::lazy_static;
use libc::c_void;
use parking_lot::{Condvar, Mutex};

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

#[cfg(feature = "debug-console")]
fn free_console() {
    use windows::Win32::System::Console::FreeConsole;

    unsafe {
        FreeConsole();
    }
}

lazy_static! {
    static ref OPERATION: Condvar = Condvar::new();
    static ref OPERATION_MUTEX: Mutex<()> = Mutex::new(());
}

fn main() {
    #[cfg(feature = "debug-console")]
    alloc_console();

    let mut modules = Module::get_all();
    let module = modules
        .iter_mut()
        .find(|x| x.filename().as_deref() == Some("HITMAN3.exe"))
        .expect("failed to find module");

    {
        let mut success = true;
        let mut loaded_libraries = vec![];

        let suspender = ThreadSuspender::new().expect("Failed to create thread suspender");
        for hook_library in [
            rendering::hook_library(),
            game::zrender::hook_library(),
            game::zapplication_engine_win32::hook_library(),
        ] {
            if let Err(error) = (hook_library.enable)(module) {
                success = false;
                println!("Failed to enable hook library: {error}");
                break;
            } else {
                loaded_libraries.push(hook_library);
            }
        }
        mem::drop(suspender);

        if success {
            OPERATION.notify_all();
            OPERATION.wait(&mut OPERATION_MUTEX.lock());
        }

        let suspender = ThreadSuspender::new().expect("Failed to create thread suspender");
        for hook_library in &loaded_libraries {
            if let Err(error) = (hook_library.disable)() {
                println!("Failed to disable hook library: {error}");
            }
        }
        mem::drop(suspender);
    }

    #[cfg(feature = "debug-console")]
    free_console();
    OPERATION.notify_all();
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "system" fn load(_: *mut c_void, _: *mut c_void) {
    thread::spawn(main);
    OPERATION.wait(&mut OPERATION_MUTEX.lock());
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "system" fn unload(_: *mut c_void, _: *mut c_void) {
    OPERATION.notify_all();
    OPERATION.wait(&mut OPERATION_MUTEX.lock());
}
