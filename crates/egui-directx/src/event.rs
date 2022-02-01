use windows::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::ScreenToClient,
    UI::{
        Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT},
        WindowsAndMessaging::GetMessagePos,
    },
};

use crate::win32::{xparam, yparam};

pub fn get_pos(hwnd: HWND, pixels_per_point: f32) -> egui::Pos2 {
    unsafe {
        let point = GetMessagePos();
        let mut point = POINT {
            x: xparam(point) as i32,
            y: yparam(point) as i32,
        };
        ScreenToClient(hwnd, &mut point);
        egui::pos2(
            point.x as f32 / pixels_per_point,
            point.y as f32 / pixels_per_point,
        )
    }
}

fn key_state(vkey: u16) -> bool {
    unsafe { (GetKeyState(vkey as _) & (1 << 15)) == (1 << 15) }
}

pub fn get_modifiers() -> egui::Modifiers {
    egui::Modifiers {
        alt: key_state(VK_MENU),
        ctrl: key_state(VK_CONTROL),
        shift: key_state(VK_SHIFT),
        mac_cmd: false,
        command: key_state(VK_CONTROL),
    }
}
