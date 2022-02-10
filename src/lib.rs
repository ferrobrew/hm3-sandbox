mod detouring;
mod game;
mod rendering;

use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

use crate::detouring::prelude::*;
use c_string::c_str;
use lazy_static::lazy_static;

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
    static ref OPERATION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
    static ref IS_RUNNING: AtomicBool = AtomicBool::new(false);
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
        let mut loaded_libraries = vec![];

        {
            let threads = ThreadGroup::new().expect("Failed to create thread group");

            threads.suspend();
            for hook_library in [
                rendering::hook_library(),
                game::zrender::hook_library(),
                game::zapplication_engine_win32::hook_library(),
            ] {
                if let Err(error) = (hook_library.enable)(module) {
                    println!("Failed to enable hook library: {error}");
                    IS_RUNNING.store(false, Ordering::Relaxed);
                    break;
                } else {
                    loaded_libraries.push(hook_library);
                }
            }
            threads.resume();
            OPERATION_IN_PROGRESS.store(false, Ordering::Relaxed);
        }

        wait_for_bool(&IS_RUNNING);

        {
            let threads = ThreadGroup::new().expect("Failed to create thread group");

            threads.suspend();
            for hook_library in &loaded_libraries {
                if let Err(error) = (hook_library.disable)() {
                    println!("Failed to disable hook library: {error}");
                }
            }
            threads.resume();
        }
    }

    #[cfg(feature = "debug-console")]
    free_console();
    OPERATION_IN_PROGRESS.store(false, Ordering::Relaxed);
}

fn wait_for_bool(bool: &AtomicBool) {
    while bool.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(10_u64));
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "system" fn load(_: usize) {
    if !IS_RUNNING.load(Ordering::Relaxed) {
        IS_RUNNING.store(true, Ordering::Relaxed);
        OPERATION_IN_PROGRESS.store(true, Ordering::Relaxed);
        thread::spawn(main);
        wait_for_bool(&OPERATION_IN_PROGRESS);
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "system" fn unload(_: usize) {
    IS_RUNNING.store(false, Ordering::Relaxed);
    OPERATION_IN_PROGRESS.store(true, Ordering::Relaxed);
    wait_for_bool(&OPERATION_IN_PROGRESS);
}
