use std::{mem, ptr};

use anyhow::{Context, Result};
use windows::Win32::Graphics::{
    Direct3D12::{
        ID3D12Device, ID3D12Resource, D3D12_HEAP_FLAG_NONE, D3D12_HEAP_PROPERTIES,
        D3D12_HEAP_TYPE_UPLOAD, D3D12_RESOURCE_DESC, D3D12_RESOURCE_DIMENSION_BUFFER,
        D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_GENERIC_READ,
        D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
    },
    Dxgi::Common::{DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC},
};

pub struct Buffer<T> {
    capacity: usize,
    resource: ID3D12Resource,
    pointer: *mut T,
}

impl<T> Buffer<T> {
    pub fn new(device: &ID3D12Device, capacity: usize) -> Result<Buffer<T>> {
        let resource = unsafe {
            let desc = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                Alignment: 0,
                Width: (mem::size_of::<T>() * capacity) as u64,
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

            let props = D3D12_HEAP_PROPERTIES {
                Type: D3D12_HEAP_TYPE_UPLOAD,
                ..Default::default()
            };

            let mut value: Option<ID3D12Resource> = None;
            device
                .CreateCommittedResource(
                    &props,
                    D3D12_HEAP_FLAG_NONE,
                    &desc,
                    D3D12_RESOURCE_STATE_GENERIC_READ,
                    ptr::null(),
                    &mut value,
                )
                .context("Failed to create placed buffer resource")?;
            value.unwrap()
        };

        let mut pointer: *mut T = ptr::null_mut();
        unsafe {
            resource
                .Map(0, ptr::null(), mem::transmute(&mut pointer))
                .context("Failed to map buffer resource")?;
        }

        Ok(Buffer::<T> {
            capacity,
            resource,
            pointer,
        })
    }

    pub fn handle(&self) -> &ID3D12Resource {
        &self.resource
    }

    pub fn get_ptr(&self, offset: usize) -> *mut T {
        if offset >= self.capacity {
            panic!("Attempted to index out of bounds buffer object");
        }

        unsafe { self.pointer.add(offset) }
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        unsafe { self.resource.Unmap(0, ptr::null()) }
    }
}
