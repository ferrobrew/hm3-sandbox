use std::{
    mem, ptr,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use gpu_allocator::{
    d3d12::{AllocationCreateDesc, Allocator},
    MemoryLocation,
};
use windows::Win32::Graphics::{
    Direct3D12::{
        ID3D12Device, ID3D12Heap, ID3D12Resource, D3D12_RESOURCE_DESC,
        D3D12_RESOURCE_DIMENSION_BUFFER, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATES,
        D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
    },
    Dxgi::Common::{DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC},
};

use super::allocated_resource::AllocatedResource;

pub struct Buffer<T> {
    capacity: u64,
    resource: AllocatedResource,
    pointer: *mut T,
}

impl<T> Buffer<T> {
    pub fn new(
        device: &ID3D12Device,
        allocator: Arc<Mutex<Allocator>>,
        capacity: u64,
        state: D3D12_RESOURCE_STATES,
    ) -> Result<Buffer<T>> {
        let desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: std::mem::size_of::<T>() as u64 * capacity,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_UNKNOWN,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: D3D12_RESOURCE_FLAG_NONE,
        };

        let allocation = unsafe {
            let mut allocator = allocator.lock().unwrap();
            allocator.allocate(&AllocationCreateDesc::from_d3d12_resource_desc(
                mem::transmute(device.clone()),
                mem::transmute(&desc),
                "buffer",
                MemoryLocation::CpuToGpu,
            ))?
        };

        let handle = unsafe {
            let mut value: Option<ID3D12Resource> = None;
            let heap: &ID3D12Heap = mem::transmute(&allocation.heap());
            let result = device
                .CreatePlacedResource(
                    heap,
                    allocation.offset(),
                    &desc,
                    state,
                    ptr::null(),
                    &mut value,
                )
                .context("Failed to create placed buffer resource");

            if let Err(error) = result {
                let mut allocator = allocator.lock().unwrap();
                allocator.free(allocation).unwrap();
                return Err(error);
            };

            value.unwrap()
        };

        let mut pointer: *mut T = ptr::null_mut();
        unsafe {
            let result = handle
                .Map(0, ptr::null(), mem::transmute(&mut pointer))
                .context("Failed to map buffer resource");

            if let Err(error) = result {
                let mut allocator = allocator.lock().unwrap();
                allocator.free(allocation).unwrap();
                return Err(error);
            };
        }

        Ok(Buffer::<T> {
            capacity,
            resource: AllocatedResource::new(allocator, allocation, handle),
            pointer,
        })
    }

    pub fn resource_handle(&self) -> ID3D12Resource {
        self.resource.handle()
    }

    pub fn pointer(&self) -> *mut T {
        self.pointer
    }
}
