mod console;
pub mod overlay;

use crate::{detouring::prelude::*, game::zrender::RENDER_MANAGER, HookLibrary};
use anyhow::Result;
use std::ptr;
use windows::{
    core::{Interface, HRESULT},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, PSTR, WPARAM},
        Graphics::{
            Direct3D::D3D_FEATURE_LEVEL_12_0,
            Direct3D12::{
                D3D12CreateDevice, ID3D12CommandAllocator, ID3D12CommandAllocatorVtbl,
                ID3D12CommandList, ID3D12CommandListVtbl, ID3D12CommandQueue,
                ID3D12CommandQueueVtbl, ID3D12Device, ID3D12DeviceVtbl,
                D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_DESC,
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

use self::overlay::OVERLAY;

#[allow(dead_code)]
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
        let flags = DXGI_CREATE_FACTORY_DEBUG;
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
    static PRESENT_DETOUR: extern "system" fn(IDXGISwapChain, u32, u32) -> HRESULT;
    static RESIZE_BUFFERS_DETOUR: extern "system" fn(IDXGISwapChain, u32, u32, u32, DXGI_FORMAT, u32) -> HRESULT;
    static RESIZE_TARGET_DETOUR: extern "system" fn(IDXGISwapChain, *const DXGI_MODE_DESC) -> HRESULT;
    static EXECUTE_COMMAND_LISTS:  extern "system" fn(ID3D12CommandQueue, u32, *const *mut ID3D12CommandList);
}

fn present(this: IDXGISwapChain, syncinterval: u32, flags: u32) -> windows::core::HRESULT {
    unsafe {
        if let (Some(render_manager), Ok(device), Ok(swap_chain)) = (
            RENDER_MANAGER,
            this.GetDevice::<ID3D12Device>(),
            this.cast::<IDXGISwapChain4>(),
        ) {
            let command_queue = &(*(*render_manager).device).command_queues[0].command_queue;
            OVERLAY
                .lock()
                .unwrap()
                .render(&device, command_queue, &swap_chain);
        }
        PRESENT_DETOUR.call(this, syncinterval, flags)
    }
}

fn resize_buffers(
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
    OVERLAY.lock().unwrap().resize(&|| {
        RESIZE_BUFFERS_DETOUR.call(
            this.clone(),
            buffercount,
            width,
            height,
            newformat,
            swapchainflags,
        )
    })
}

fn resize_target(this: IDXGISwapChain, pnewtargetparameters: *const DXGI_MODE_DESC) -> HRESULT {
    #[cfg(feature = "debug-logging")]
    println!(
        "resize_target(pnewtargetparameters: 0x{:X})",
        pnewtargetparameters as usize
    );
    RESIZE_TARGET_DETOUR.call(this, pnewtargetparameters)
}

fn enable(_: &mut Module) -> Result<()> {
    unsafe {
        let vtables = get_vtables()?;

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

        #[cfg(feature = "debug-logging")]
        println!("Hooked DX12 vtbls:\n{:?}", vtables);
    }
    Ok(())
}

fn disable() -> Result<()> {
    unsafe {
        PRESENT_DETOUR.disable()?;
        RESIZE_BUFFERS_DETOUR.disable()?;
        RESIZE_TARGET_DETOUR.disable()?;

        #[cfg(feature = "debug-logging")]
        println!("Unhooked DX12 vtbls");
    }
    Ok(())
}

pub fn hook_library() -> HookLibrary {
    HookLibrary { enable, disable }
}
