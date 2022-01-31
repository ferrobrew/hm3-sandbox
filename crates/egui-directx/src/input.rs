use std::time::Instant;

use egui::{pos2, vec2, Event, PointerButton, Pos2, RawInput, Rect};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    UI::{
        Controls::WM_MOUSELEAVE,
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            GetClientRect, USER_DEFAULT_SCREEN_DPI, WHEEL_DELTA, WM_DPICHANGED, WM_LBUTTONDBLCLK,
            WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP,
            WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN,
            WM_RBUTTONUP, WM_SIZE,
        },
    },
};

use crate::{event, win32::Win32};

pub struct WindowInput {
    hwnd: HWND,
    pos: Pos2,
    raw: RawInput,
    start_time: Instant,
}

impl WindowInput {
    pub fn new(hwnd: HWND) -> Self {
        let mut raw = RawInput::default();

        let (width, height) = unsafe {
            let mut rect = RECT::default();
            GetClientRect(hwnd, &mut rect);
            (
                (rect.right - rect.left) as f32,
                (rect.bottom - rect.top) as f32,
            )
        };
        raw.screen_rect = Some(Rect {
            min: pos2(0.0, 0.0),
            max: pos2(width, height),
        });

        let dpi_scale = unsafe { GetDpiForWindow(hwnd) as f32 / (USER_DEFAULT_SCREEN_DPI as f32) };
        raw.pixels_per_point = Some(dpi_scale);

        Self {
            hwnd,
            pos: Pos2::default(),
            raw,
            start_time: Instant::now(),
        }
    }

    fn add_mouse_event(&mut self, button: PointerButton, pressed: bool) {
        let pos = event::get_pos(self.hwnd);
        let modifiers = event::get_modifiers();
        self.raw.events.push(Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers,
        });
    }

    pub fn get_input(&mut self) -> RawInput {
        self.raw.time = Some(self.start_time.elapsed().as_secs_f64());
        self.raw.modifiers = event::get_modifiers();
        self.raw.take()
    }

    pub fn wnd_proc(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> bool {
        return match msg {
            WM_DPICHANGED => {
                let dpi_scale = (wparam.loword() as f32) / (USER_DEFAULT_SCREEN_DPI as f32);
                self.raw.pixels_per_point = Some(dpi_scale);
                false
            }
            WM_SIZE => {
                let width = lparam.loword() as f32;
                let height = lparam.hiword() as f32;
                self.raw.screen_rect = Some(Rect {
                    min: pos2(0.0, 0.0),
                    max: pos2(width, height),
                });
                false
            }
            WM_MOUSEMOVE => {
                let pos = event::get_pos(self.hwnd);
                if pos != self.pos {
                    self.raw.events.push(Event::PointerMoved(pos));
                    self.pos = pos;
                }
                true
            }
            WM_MOUSELEAVE => {
                self.raw.events.push(Event::PointerGone);
                true
            }
            WM_MOUSEWHEEL => {
                let y = wparam.yparam() as f32 / WHEEL_DELTA as f32;
                self.raw.events.push(Event::Scroll(vec2(0.0, y)));
                true
            }
            WM_MOUSEHWHEEL => {
                let x = wparam.yparam() as f32 / WHEEL_DELTA as f32;
                self.raw.events.push(Event::Scroll(vec2(x, 0.0)));
                true
            }
            WM_LBUTTONDOWN => {
                self.add_mouse_event(PointerButton::Primary, true);
                true
            }
            WM_LBUTTONDBLCLK => {
                self.add_mouse_event(PointerButton::Primary, true);
                true
            }
            WM_LBUTTONUP => {
                self.add_mouse_event(PointerButton::Primary, false);
                true
            }
            WM_RBUTTONDOWN => {
                self.add_mouse_event(PointerButton::Secondary, true);
                true
            }
            WM_RBUTTONDBLCLK => {
                self.add_mouse_event(PointerButton::Secondary, true);
                true
            }
            WM_RBUTTONUP => {
                self.add_mouse_event(PointerButton::Secondary, false);
                true
            }
            WM_MBUTTONDOWN => {
                self.add_mouse_event(PointerButton::Middle, true);
                true
            }
            WM_MBUTTONDBLCLK => {
                self.add_mouse_event(PointerButton::Middle, true);
                true
            }
            WM_MBUTTONUP => {
                self.add_mouse_event(PointerButton::Middle, false);
                true
            }
            _ => false,
        };
    }
}
