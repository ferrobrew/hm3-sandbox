use crate::{detouring::prelude::*, game::zrender::RENDER_MANAGER};
use anyhow::Result;
use egui::Color32;
use egui_directx::{self, Painter, PainterDX12};
use std::{mem, ptr};
use windows::{
    core::{Interface, HRESULT},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, PSTR, WPARAM},
        Graphics::{
            Direct3D::D3D_FEATURE_LEVEL_12_0,
            Direct3D12::{
                D3D12CreateDevice, D3D12GetDebugInterface, ID3D12CommandAllocator,
                ID3D12CommandAllocatorVtbl, ID3D12CommandList, ID3D12CommandListVtbl,
                ID3D12CommandQueue, ID3D12CommandQueueVtbl, ID3D12Debug, ID3D12Device,
                ID3D12DeviceVtbl, D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_DESC,
            },
            Dxgi::{
                Common::{
                    DXGI_FORMAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_MODE_DESC, DXGI_SAMPLE_DESC,
                },
                CreateDXGIFactory2, IDXGIAdapter, IDXGIAdapterVtbl, IDXGIFactory2,
                IDXGIFactory2Vtbl, IDXGISwapChain, IDXGISwapChain1Vtbl, IDXGISwapChain4,
                DXGI_CREATE_FACTORY_DEBUG, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_DISCARD,
                DXGI_USAGE_RENDER_TARGET_OUTPUT,
            },
        },
        System::LibraryLoader::GetModuleHandleA,
        UI::WindowsAndMessaging::{
            CreateWindowExA, DefWindowProcA, DestroyWindow, RegisterClassExA, UnregisterClassA,
            CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, WNDCLASSEXA, WS_OVERLAPPEDWINDOW,
        },
    },
};

#[derive(Debug)]
struct VTables {
    pub idxgifactory2_vtbl: *const IDXGIFactory2Vtbl,
    pub idxgiadapter_vtbl: *const IDXGIAdapterVtbl,
    pub id3d12_device_vtbl: *const ID3D12DeviceVtbl,
    pub id3d12_command_queue_vtbl: *const ID3D12CommandQueueVtbl,
    pub id3d12_command_allocator_vtbl: *const ID3D12CommandAllocatorVtbl,
    pub id3d12_command_list_vtbl: *const ID3D12CommandListVtbl,
    pub idxgiswapchain1_vtbl: *const IDXGISwapChain1Vtbl,
}

unsafe extern "system" fn def_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcA(hwnd, msg, wparam, lparam)
}

fn get_vtables() -> Result<VTables> {
    unsafe {
        let mut flags = 0;

        #[cfg(feature = "debug-logging")]
        {
            let mut debug: Option<ID3D12Debug> = None;
            if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and(debug) {
                debug.EnableDebugLayer();
            }

            flags &= DXGI_CREATE_FACTORY_DEBUG;
        }

        let factory: IDXGIFactory2 = CreateDXGIFactory2(flags)?;
        let adapter: IDXGIAdapter = factory.EnumAdapters(0)?;
        let device: ID3D12Device = {
            let mut x = None;
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_12_0, &mut x)?;
            x.unwrap()
        };

        let desc = D3D12_COMMAND_QUEUE_DESC::default();
        let command_queue: ID3D12CommandQueue = device.CreateCommandQueue(&desc)?;
        let command_allocator: ID3D12CommandAllocator =
            device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let command_list: ID3D12CommandList = device.CreateCommandList(
            0,
            D3D12_COMMAND_LIST_TYPE_DIRECT,
            &command_allocator,
            None,
        )?;

        let window_class = WNDCLASSEXA {
            cbSize: std::mem::size_of::<WNDCLASSEXA>() as _,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(def_wnd_proc),
            hInstance: GetModuleHandleA(None),
            lpszClassName: PSTR(b"hm3-sandbox\0".as_ptr() as _),
            ..Default::default()
        };

        RegisterClassExA(&window_class);

        let window = CreateWindowExA(
            Default::default(),
            window_class.lpszClassName,
            PSTR("hm3-sandbox\0".as_ptr() as _),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            256,
            256,
            None,
            None,
            window_class.hInstance,
            ptr::null(),
        );

        let desc = DXGI_SWAP_CHAIN_DESC1 {
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            ..Default::default()
        };
        let swap_chain =
            factory.CreateSwapChainForHwnd(&command_queue, &window, &desc, ptr::null(), None)?;

        DestroyWindow(window);
        UnregisterClassA(window_class.lpszClassName, window_class.hInstance);

        Ok(VTables {
            idxgifactory2_vtbl: Interface::vtable(&factory),
            idxgiadapter_vtbl: Interface::vtable(&adapter),
            id3d12_device_vtbl: Interface::vtable(&device),
            id3d12_command_queue_vtbl: Interface::vtable(&command_queue),
            id3d12_command_allocator_vtbl: Interface::vtable(&command_allocator),
            id3d12_command_list_vtbl: Interface::vtable(&command_list),
            idxgiswapchain1_vtbl: Interface::vtable(&swap_chain),
        })
    }
}

static_detour! {
    pub static PRESENT_DETOUR: extern "system" fn(IDXGISwapChain, u32, u32) -> HRESULT;
    pub static RESIZE_BUFFERS_DETOUR: extern "system" fn(IDXGISwapChain, u32, u32, u32, DXGI_FORMAT, u32) -> HRESULT;
    pub static RESIZE_TARGET_DETOUR: extern "system" fn(IDXGISwapChain, *const DXGI_MODE_DESC) -> HRESULT;
    pub static EXECUTE_COMMAND_LISTS:  extern "system" fn(ID3D12CommandQueue, u32, *const *mut ID3D12CommandList);
}

static mut PAINTER: Option<PainterDX12> = None;
static mut CURRENT_COMMAND_QUEUE: Option<ID3D12CommandQueue> = None;

pub fn present(this: IDXGISwapChain, syncinterval: u32, flags: u32) -> windows::core::HRESULT {
    #[cfg(feature = "debug-logging")]
    println!("present(syncinterval: {}, flags: {})", syncinterval, flags);
    unsafe {
        if let Ok(swap_chain) = this.cast::<IDXGISwapChain4>() {
            if let Ok(device) = swap_chain.GetDevice::<ID3D12Device>() {
                let input = egui::RawInput::default();
                let mut ctx = egui::CtxRef::default();
                let (output, shapes) = ctx.run(input, |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame {
                            fill: Color32::TRANSPARENT,
                            ..Default::default()
                        })
                        .show(&ctx, |ui| {
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

                if let Some(render_manager) = RENDER_MANAGER {
                    if PAINTER.is_none() {
                        let command_queue =
                            &(*(*render_manager).device).command_queues[0].command_queue;
                        PAINTER = PainterDX12::new(device, command_queue.clone(), swap_chain).ok();
                    }

                    if let Some(painter) = &mut PAINTER {
                        painter.upload_egui_texture(&ctx.font_image());
                        painter.paint_meshes(ctx.tessellate(shapes), 1.0);
                    }
                }
            }
        }
        PRESENT_DETOUR.call(this, syncinterval, flags)
    }
}

pub fn resize_buffers(
    this: IDXGISwapChain,
    buffercount: u32,
    width: u32,
    height: u32,
    newformat: DXGI_FORMAT,
    swapchainflags: u32,
) -> HRESULT {
    #[cfg(feature = "debug-logging")]
    println!(
        "resize_buffers(buffercount: {}, width: {}, height: {}, newformat: {}, swapchainflags: {})",
        buffercount, width, height, newformat, swapchainflags
    );

    unsafe {
        let detour = || {
            RESIZE_BUFFERS_DETOUR.call(
                this.clone(),
                buffercount,
                width,
                height,
                newformat,
                swapchainflags,
            )
        };

        if let Some(painter) = &mut PAINTER {
            painter
                .resize_buffers(&detour)
                .expect("Failed to resize buffers")
        } else {
            detour()
        }
    }
}

pub fn resize_target(this: IDXGISwapChain, pnewtargetparameters: *const DXGI_MODE_DESC) -> HRESULT {
    #[cfg(feature = "debug-logging")]
    println!(
        "resize_target(pnewtargetparameters: 0x{:X})",
        pnewtargetparameters as usize
    );
    unsafe { RESIZE_TARGET_DETOUR.call(this, pnewtargetparameters) }
}

pub fn execute_command_lists(
    this: ID3D12CommandQueue,
    num_command_lists: u32,
    command_lists: *const *mut ID3D12CommandList,
) {
    #[cfg(feature = "debug-logging")]
    println!(
        "execute_command_lists(num_command_lists: {}, command_lists: 0x{:X})",
        num_command_lists, command_lists as usize
    );
    unsafe {
        CURRENT_COMMAND_QUEUE = Some(this.clone());
        EXECUTE_COMMAND_LISTS.call(this, num_command_lists, command_lists)
    }
}

pub fn hook(module: &Module) -> Result<()> {
    unsafe {
        let vtables = get_vtables()?;
        println!("{:?}", vtables);

        PRESENT_DETOUR.initialize(
            std::mem::transmute((*vtables.idxgiswapchain1_vtbl).8),
            present,
        )?;
        PRESENT_DETOUR.enable()?;

        RESIZE_BUFFERS_DETOUR.initialize(
            std::mem::transmute((*vtables.idxgiswapchain1_vtbl).13),
            resize_buffers,
        )?;
        RESIZE_BUFFERS_DETOUR.enable()?;

        RESIZE_TARGET_DETOUR.initialize(
            std::mem::transmute((*vtables.idxgiswapchain1_vtbl).14),
            resize_target,
        )?;
        RESIZE_TARGET_DETOUR.enable()?;

        EXECUTE_COMMAND_LISTS.initialize(
            std::mem::transmute((*vtables.id3d12_command_queue_vtbl).10),
            execute_command_lists,
        )?;
        EXECUTE_COMMAND_LISTS.enable()?;
    }
    println!("Detoured rendering");
    Ok(())
}
