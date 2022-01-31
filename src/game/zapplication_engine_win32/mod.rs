use anyhow::Result;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};

use crate::{detouring::prelude::*, rendering::overlay::OVERLAY};

#[detour {
    name = "ZApplicationEngineWin32::WndProc",
    pattern = "48 89 5C 24 ? 48 89 74 24 ? 48 89 7C 24 ? 55 41 54 41 55 41 56 41 57 48 8D 6C 24 ? 48 81 EC ? ? ? ? 4C 8B 65 7F",
}]
pub fn wnd_proc(this: usize, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if OVERLAY.lock().unwrap().wnd_proc(hwnd, msg, wparam, lparam) {
        LRESULT(0)
    } else {
        unsafe { WND_PROC.call(this, hwnd, msg, wparam, lparam) }
    }
}

pub fn hook(module: &mut Module) -> Result<()> {
    for binder in [&WND_PROC_BINDER] {
        binder.bind(module)?;
        binder.enable()?
    }

    Ok(())
}
