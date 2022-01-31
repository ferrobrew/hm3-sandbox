use std::mem;

use crate::detouring::prelude::*;
use anyhow::{Context, Result};
use windows::Win32::{
    Foundation::HANDLE,
    Graphics::Direct3D12::{ID3D12CommandQueue, ID3D12Device, ID3D12Fence},
    System::Threading::RTL_CRITICAL_SECTION,
};

#[repr(C)]
pub struct ZRenderCommandQueue {
    pub critical_section: RTL_CRITICAL_SECTION,
    pub command_queue: ID3D12CommandQueue,
    pub fence: ID3D12Fence,
    pub fence_value: u64,
    pub event: HANDLE,
}

#[repr(C)]
pub struct ZRenderSwapChain {}

#[repr(C)]
pub struct ZRenderDevice {
    pub pad0: [u8; 0x410],                        // 0x0
    pub swap_chain: *const ZRenderSwapChain,      // 0x410
    pub pad418: [u8; 0x8],                        // 0x418
    pub device: ID3D12Device,                     // 0x420
    pub pad428: [u8; 0x30E92F8],                  // 0x428
    pub command_queues: [ZRenderCommandQueue; 4], // 0x30E9720
}

#[repr(C)]
pub struct ZRenderManager {
    pub pad0: [u8; 0x14178],          // 0x0
    pub device: *const ZRenderDevice, // 0x14178
}

pub static mut RENDER_MANAGER: Option<*const ZRenderManager> = None;

pub fn hook(module: &mut Module) -> Result<()> {
    unsafe {
        let render_manager = module
            .scan_for_relative_callsite(
                "48 8D 0D ? ? ? ? E8 ? ? ? ? 48 8B 1D ? ? ? ? 48 8D 4B 60 FF 15",
                3,
            )
            .context("Failed to find ZRenderManager")?;

        RENDER_MANAGER = Some(mem::transmute(render_manager));
        println!("Hooked render_manager: 0x{:x}", *render_manager);
    }
    Ok(())
}
