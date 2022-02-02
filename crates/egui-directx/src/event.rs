use windows::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::ScreenToClient,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::GetMessagePos},
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

pub fn to_key(vkey: u16) -> Option<egui::Key> {
    match vkey {
        VK_DOWN => Some(egui::Key::ArrowDown),
        VK_LEFT => Some(egui::Key::ArrowLeft),
        VK_RIGHT => Some(egui::Key::ArrowRight),
        VK_UP => Some(egui::Key::ArrowUp),
        VK_ESCAPE => Some(egui::Key::Escape),
        VK_TAB => Some(egui::Key::Tab),
        VK_BACK => Some(egui::Key::Backspace),
        VK_RETURN => Some(egui::Key::Enter),
        VK_SPACE => Some(egui::Key::Space),
        VK_INSERT => Some(egui::Key::Insert),
        VK_DELETE => Some(egui::Key::Delete),
        VK_HOME => Some(egui::Key::Home),
        VK_END => Some(egui::Key::End),
        VK_PRIOR => Some(egui::Key::PageUp),
        VK_NEXT => Some(egui::Key::PageDown),
        VK_NUMPAD0 | 0x30 => Some(egui::Key::Num0),
        VK_NUMPAD1 | 0x31 => Some(egui::Key::Num1),
        VK_NUMPAD2 | 0x32 => Some(egui::Key::Num2),
        VK_NUMPAD3 | 0x33 => Some(egui::Key::Num3),
        VK_NUMPAD4 | 0x34 => Some(egui::Key::Num4),
        VK_NUMPAD5 | 0x35 => Some(egui::Key::Num5),
        VK_NUMPAD6 | 0x36 => Some(egui::Key::Num6),
        VK_NUMPAD7 | 0x37 => Some(egui::Key::Num7),
        VK_NUMPAD8 | 0x38 => Some(egui::Key::Num8),
        VK_NUMPAD9 | 0x39 => Some(egui::Key::Num9),
        VK_A => Some(egui::Key::A),
        VK_B => Some(egui::Key::B),
        VK_C => Some(egui::Key::C),
        VK_D => Some(egui::Key::D),
        VK_E => Some(egui::Key::E),
        VK_F => Some(egui::Key::F),
        VK_G => Some(egui::Key::G),
        VK_H => Some(egui::Key::H),
        VK_I => Some(egui::Key::I),
        VK_J => Some(egui::Key::J),
        VK_K => Some(egui::Key::K),
        VK_L => Some(egui::Key::L),
        VK_M => Some(egui::Key::M),
        VK_N => Some(egui::Key::N),
        VK_O => Some(egui::Key::O),
        VK_P => Some(egui::Key::P),
        VK_Q => Some(egui::Key::Q),
        VK_R => Some(egui::Key::R),
        VK_S => Some(egui::Key::S),
        VK_T => Some(egui::Key::T),
        VK_U => Some(egui::Key::U),
        VK_V => Some(egui::Key::V),
        VK_W => Some(egui::Key::W),
        VK_X => Some(egui::Key::X),
        VK_Y => Some(egui::Key::Y),
        VK_Z => Some(egui::Key::Z),
        _ => None,
    }
}
