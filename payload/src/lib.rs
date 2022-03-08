mod console;
mod detouring;
mod game;
mod rendering;

use std::{thread, time::Duration};

use crate::detouring::prelude::*;
use anyhow::Context;
use c_string::c_str;
use lazy_static::lazy_static;
use parking_lot::{Condvar, Mutex};
use re_utilities::{module::Module, ThreadSuspender};

pub use console::{MessageType, CONSOLE};

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

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "debug-console")]
    alloc_console();

    let mut module = Module::get_all()
        .find(|x| {
            x.filename()
                .unwrap_or_default()
                .to_uppercase()
                .contains(&"HITMAN3.EXE")
        })
        .context("Failed to find game module")?;

    let loaded_libraries = ThreadSuspender::for_block(|| {
        [
            rendering::hook_library(),
            game::zrender::hook_library(),
            game::zapplication_engine_win32::hook_library(),
        ]
        .into_iter()
        .map(|hl| (hl.enable)(&mut module).map(|_| hl))
        .collect::<anyhow::Result<Vec<_>>>()
    })?;

    {
        let mut console = CONSOLE.lock().unwrap();
        console.push_back_info("Hello from hm3-sandbox!".into());
    }

    OPERATION.notify_all();
    OPERATION.wait(&mut OPERATION_MUTEX.lock());

    ThreadSuspender::for_block(|| {
        for hook_library in &loaded_libraries {
            (hook_library.disable)()?;
        }
        Ok(())
    })?;

    #[cfg(feature = "debug-console")]
    println!("Delaying exit...");
    thread::sleep(Duration::new(1, 0));

    #[cfg(feature = "debug-console")]
    free_console();
    OPERATION.notify_all();

    Ok(())
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "system" fn load(_: *mut u64, _: *mut u64) {
    thread::spawn(|| main().unwrap());
    OPERATION.wait(&mut OPERATION_MUTEX.lock());
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "system" fn unload(_: *mut u64, _: *mut u64) {
    OPERATION.notify_all();
    OPERATION.wait(&mut OPERATION_MUTEX.lock());
}
