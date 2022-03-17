use anyhow::Result;
use egui::Color32;
use egui_directx::{Painter, PainterDX12};
use std::ptr;
use windows::{
    core::Interface,
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct3D::D3D_FEATURE_LEVEL_12_0,
            Direct3D12::{
                D3D12CreateDevice, D3D12GetDebugInterface, ID3D12CommandQueue, ID3D12Debug1,
                ID3D12Device, D3D12_COMMAND_QUEUE_DESC,
            },
            Dxgi::{
                Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC},
                CreateDXGIFactory2, IDXGIAdapter, IDXGIFactory4, IDXGISwapChain4,
                DXGI_CREATE_FACTORY_DEBUG, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_DISCARD,
                DXGI_USAGE_RENDER_TARGET_OUTPUT,
            },
        },
    },
};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::windows::WindowExtWindows,
    window::{Window, WindowBuilder},
};

struct App {
    _device: ID3D12Device,
    _command_queue: ID3D12CommandQueue,
    swap_chain: IDXGISwapChain4,
    painter: PainterDX12,
}

impl App {
    fn new(window: &Window) -> Result<App> {
        let mut flags = 0;

        #[cfg(debug_assertions)]
        unsafe {
            let mut d3d_debug = None;

            if D3D12GetDebugInterface::<ID3D12Debug1>(&mut d3d_debug).is_ok() {
                d3d_debug.unwrap().EnableDebugLayer();
            }

            flags &= DXGI_CREATE_FACTORY_DEBUG;
        }

        unsafe {
            let factory: IDXGIFactory4 = CreateDXGIFactory2(flags)?;
            let adapter: IDXGIAdapter = factory.EnumAdapters(0)?;
            let device: ID3D12Device = {
                let mut x = None;
                D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_12_0, &mut x)?;
                x.unwrap()
            };
            let command_queue: ID3D12CommandQueue =
                device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC::default())?;
            let swap_chain: IDXGISwapChain4 = factory
                .CreateSwapChainForHwnd(
                    &command_queue,
                    HWND(window.hwnd() as _),
                    &DXGI_SWAP_CHAIN_DESC1 {
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                        BufferCount: 2,
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                        ..Default::default()
                    },
                    ptr::null(),
                    None,
                )?
                .cast::<IDXGISwapChain4>()?;

            let painter =
                PainterDX12::new(device.clone(), command_queue.clone(), swap_chain.clone())?;

            Ok(App {
                _device: device,
                _command_queue: command_queue,
                swap_chain,
                painter,
            })
        }
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        Ok(self.painter.resize_buffers(|| unsafe {
            self.swap_chain
                .ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, 0)
        })??)
    }

    fn render(&mut self) -> Result<()> {
        let input = egui::RawInput::default();
        let mut ctx = egui::CtxRef::default();

        let (_, shapes) = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame {
                    fill: Color32::RED,
                    ..Default::default()
                })
                .show(ctx, |ui| {
                    ui.label("Hello world!");
                    ui.label("Hello world!");
                    ui.label("Hello world!");
                    ui.label("Hello world!");
                    ui.label("Hello world!");
                    ui.label("Hello world!");
                    ui.label("Hello world!");
                    if ui.button("Click me").clicked() {
                        // take some action here
                    }
                });
        });

        self.painter.upload_egui_texture(&ctx.font_image());
        self.painter.paint_meshes(ctx.tessellate(shapes), 1.0)?;

        unsafe {
            self.swap_chain.Present(1, 0).expect("Failed to present");
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;
    let mut app = App::new(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => match event {
                WindowEvent::Resized(PhysicalSize { width, height }) => {
                    app.resize(width, height).expect("Failed to resize app");
                }
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => (),
            },
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                app.render().expect("Failed to render app");
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => (),
        }
    });
}
