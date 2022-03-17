use std::sync::Mutex;

use egui::CtxRef;
use egui_directx::{Painter, PainterDX12, Win32, WindowInput};
use lazy_static::lazy_static;
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    Graphics::{
        Direct3D12::{ID3D12CommandQueue, ID3D12Device},
        Dxgi::IDXGISwapChain4,
    },
    UI::{
        Input::KeyboardAndMouse::{VIRTUAL_KEY, VK_OEM_3},
        WindowsAndMessaging::{WM_DPICHANGED, WM_KEYUP, WM_SIZE},
    },
};

pub struct Overlay {
    ctx: CtxRef,
    input: Option<WindowInput>,
    capture: bool,
    painter: Option<PainterDX12>,
    render: bool,
}

impl Overlay {
    pub fn new() -> Self {
        Self {
            ctx: CtxRef::default(),
            input: None,
            capture: false,
            painter: None,
            render: true,
        }
    }

    pub fn wnd_proc(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> bool {
        if self.input.is_none() {
            self.input = Some(WindowInput::new(hwnd));
        }

        if msg == WM_KEYUP && VIRTUAL_KEY(wparam.loword()) == VK_OEM_3 {
            self.capture = !self.capture;
        }

        let capture = self.capture || matches!(msg, WM_SIZE | WM_DPICHANGED);

        if let (true, Some(input)) = (capture, &mut self.input) {
            input.wnd_proc(msg, wparam, lparam)
        } else {
            false
        }
    }

    pub fn resize<F, R>(&mut self, callback: F) -> R
    where
        F: FnOnce() -> R,
    {
        if let Some(painter) = &mut self.painter {
            painter.resize_buffers(callback).unwrap()
        } else {
            callback()
        }
    }

    pub fn render(
        &mut self,
        device: &ID3D12Device,
        command_queue: &ID3D12CommandQueue,
        swap_chain: &IDXGISwapChain4,
    ) {
        if self.painter.is_none() {
            self.painter = Some(
                PainterDX12::new(device.clone(), command_queue.clone(), swap_chain.clone())
                    .unwrap(),
            );
        }

        if let (true, Some(input), Some(painter)) =
            (self.render, &mut self.input, &mut self.painter)
        {
            let ctx = &mut self.ctx;
            let input = input.get_input();

            let (_, shapes) = ctx.run(input, |ctx| {
                Self::show_title_bar(ctx, self.capture);

                if self.capture {
                    let mut console = crate::CONSOLE.lock().unwrap();
                    console.show(ctx);
                }
            });

            painter.upload_egui_texture(&ctx.font_image());
            painter
                .paint_meshes(ctx.tessellate(shapes), ctx.pixels_per_point())
                .unwrap();
        }
    }

    fn show_title_bar(ctx: &CtxRef, capture: bool) {
        let frame = egui::Frame::none();
        egui::TopBottomPanel::top("title_bar")
            .frame(frame)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(5.0);
                    ui.label("hm3-sandbox");
                    ui.small(format!(
                        "Press ~ to {} menu",
                        match capture {
                            true => "close",
                            false => "open",
                        }
                    ));
                })
            });
    }
}

lazy_static! {
    pub static ref OVERLAY: Mutex<Overlay> = Mutex::new(Overlay::new());
}
