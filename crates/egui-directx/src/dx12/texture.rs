use std::{
    mem, ptr,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Context, Result};

use egui::Color32;
use gpu_allocator::{
    d3d12::{AllocationCreateDesc, Allocator},
    MemoryLocation,
};
use windows::Win32::Graphics::{
    Direct3D12::{
        ID3D12Device, ID3D12GraphicsCommandList, ID3D12Heap, ID3D12Resource,
        D3D12_PLACED_SUBRESOURCE_FOOTPRINT, D3D12_RESOURCE_DESC, D3D12_RESOURCE_DIMENSION_BUFFER,
        D3D12_RESOURCE_DIMENSION_TEXTURE2D, D3D12_RESOURCE_STATE_COPY_DEST,
        D3D12_RESOURCE_STATE_GENERIC_READ, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        D3D12_SHADER_COMPONENT_MAPPING_ALWAYS_SET_BIT_AVOIDING_ZEROMEM_MISTAKES,
        D3D12_SHADER_COMPONENT_MAPPING_MASK, D3D12_SHADER_COMPONENT_MAPPING_SHIFT,
        D3D12_SHADER_RESOURCE_VIEW_DESC, D3D12_SHADER_RESOURCE_VIEW_DESC_0,
        D3D12_SRV_DIMENSION_TEXTURE2D, D3D12_TEX2D_SRV, D3D12_TEXTURE_COPY_LOCATION,
        D3D12_TEXTURE_COPY_LOCATION_0, D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
        D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
    },
    Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC},
};

use super::{
    allocated_resource::AllocatedResource, descriptor_heap::DescriptorHeap,
    frame_context::resource_transition, heap_resource::HeapResource,
};

pub struct Texture {
    width: u32,
    height: u32,
    texture_resource: AllocatedResource,
    resource_view: HeapResource,
    upload_resource: AllocatedResource,
    layout: D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
    data: Option<Vec<Color32>>,
}

impl Texture {
    pub fn new(
        device: &ID3D12Device,
        descriptor_heap: Arc<Mutex<DescriptorHeap>>,
        allocator: Arc<Mutex<Allocator>>,
        width: u32,
        height: u32,
    ) -> Result<Texture> {
        let texture_resource = {
            let desc = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Width: width as _,
                Height: height,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                ..Default::default()
            };

            let allocation = unsafe {
                let mut allocator = allocator.lock().unwrap();
                let desc = AllocationCreateDesc::from_d3d12_resource_desc(
                    allocator.device(),
                    mem::transmute(&desc),
                    "texture",
                    MemoryLocation::CpuToGpu,
                );
                allocator
                    .allocate(&desc)
                    .context("failed to allocate texture")?
            };

            let handle = unsafe {
                let heap: &ID3D12Heap = mem::transmute(&allocation.heap());
                let mut value: Option<ID3D12Resource> = None;
                let result = device
                    .CreatePlacedResource(
                        heap,
                        allocation.offset(),
                        &desc,
                        D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                        ptr::null(),
                        &mut value,
                    )
                    .context("failed to create placed texture resource");

                if let Err(error) = result {
                    let mut allocator = allocator.lock().unwrap();
                    allocator.free(allocation).unwrap();
                    return Err(error);
                };

                value.unwrap()
            };

            AllocatedResource::new(Arc::clone(&allocator), allocation, handle)
        };

        let resource_view = {
            let heap_handle = descriptor_heap
                .lock()
                .expect("Failed to get heap lock")
                .allocate()?;

            const fn d3d12_encode_shader_4_component_mapping(
                src0: u32,
                src1: u32,
                src2: u32,
                src3: u32,
            ) -> u32 {
                (src0 & D3D12_SHADER_COMPONENT_MAPPING_MASK)
                    | ((src1 & D3D12_SHADER_COMPONENT_MAPPING_MASK)
                        << (D3D12_SHADER_COMPONENT_MAPPING_SHIFT))
                    | ((src2 & D3D12_SHADER_COMPONENT_MAPPING_MASK)
                        << (D3D12_SHADER_COMPONENT_MAPPING_SHIFT * 2))
                    | ((src3 & D3D12_SHADER_COMPONENT_MAPPING_MASK)
                        << (D3D12_SHADER_COMPONENT_MAPPING_SHIFT * 3))
                    | D3D12_SHADER_COMPONENT_MAPPING_ALWAYS_SET_BIT_AVOIDING_ZEROMEM_MISTAKES
            }
            const fn d3d12_default_shader_4_component_mapping() -> u32 {
                d3d12_encode_shader_4_component_mapping(0, 1, 2, 3)
            }

            unsafe {
                device.CreateShaderResourceView(
                    texture_resource.handle(),
                    &D3D12_SHADER_RESOURCE_VIEW_DESC {
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
                        ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
                        Shader4ComponentMapping: d3d12_default_shader_4_component_mapping(),
                        Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                            Texture2D: D3D12_TEX2D_SRV {
                                MipLevels: 1,
                                ..Default::default()
                            },
                        },
                    },
                    heap_handle.cpu_handle(),
                );
            };

            HeapResource::new(descriptor_heap, heap_handle)
        };

        let (layout, total_bytes) = unsafe {
            let mut layouts = [D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default()];
            let mut num_rows: u32 = 0;
            let mut row_size_in_bytes: u64 = 0;
            let mut total_bytes: u64 = 0;

            device.GetCopyableFootprints(
                &texture_resource.handle().GetDesc(),
                0,
                layouts.len() as u32,
                0,
                layouts.as_mut_ptr(),
                &mut num_rows,
                &mut row_size_in_bytes,
                &mut total_bytes,
            );

            (layouts[0], total_bytes)
        };

        let upload_resource = {
            let desc = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                Width: total_bytes,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: DXGI_FORMAT_UNKNOWN,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                ..Default::default()
            };

            let allocation = unsafe {
                let mut allocator = allocator.lock().unwrap();
                let desc = AllocationCreateDesc::from_d3d12_resource_desc(
                    allocator.device(),
                    mem::transmute(&desc),
                    "upload",
                    MemoryLocation::CpuToGpu,
                );
                allocator
                    .allocate(&desc)
                    .context("failed to allocate upload buffer")?
            };

            let handle = unsafe {
                let heap: &ID3D12Heap = mem::transmute(&allocation.heap());
                let mut value: Option<ID3D12Resource> = None;
                let result = device
                    .CreatePlacedResource(
                        heap,
                        allocation.offset(),
                        &desc,
                        D3D12_RESOURCE_STATE_GENERIC_READ,
                        ptr::null(),
                        &mut value,
                    )
                    .context("failed to create placed texture resource");

                if let Err(error) = result {
                    let mut allocator = allocator.lock().unwrap();
                    allocator.free(allocation).unwrap();
                    return Err(error);
                };

                value.unwrap()
            };

            AllocatedResource::new(Arc::clone(&allocator), allocation, handle)
        };

        Ok(Texture {
            width,
            height,
            texture_resource,
            resource_view,
            upload_resource,
            layout,
            data: None,
        })
    }

    pub fn update(&mut self, data: Vec<Color32>) -> Result<()> {
        if data.len() != (self.layout.Footprint.Width * self.layout.Footprint.Height) as usize {
            return Err(anyhow!("Invalid data!"));
        }

        self.data = Some(data);
        Ok(())
    }

    pub fn bind(&mut self, command_list: &ID3D12GraphicsCommandList) -> Result<()> {
        if let Some(pixels) = self.data.take() {
            let texture_resource = self.texture_resource.handle();
            let upload_resource = self.upload_resource.handle();
            let layout = &self.layout;

            unsafe {
                let mut dst: *mut u8 = std::ptr::null_mut();

                self.upload_resource
                    .handle()
                    .Map(0, std::ptr::null(), mem::transmute(&mut dst))
                    .context("Failed to map resource")?;

                let dst = dst.add(layout.Offset as usize);
                let src = pixels.as_ptr() as *const u8;

                for y in 0..layout.Footprint.Height {
                    let dst = dst.add((y * layout.Footprint.RowPitch) as usize);
                    let src = src.add((y * layout.Footprint.Width * 4) as usize);

                    std::ptr::copy_nonoverlapping(
                        src,
                        dst as *mut u8,
                        (layout.Footprint.Width * 4) as usize,
                    );
                }

                self.upload_resource.handle().Unmap(0, std::ptr::null())
            };

            let dst_resource = texture_resource.clone();
            let dst = D3D12_TEXTURE_COPY_LOCATION {
                pResource: Some(dst_resource),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                ..Default::default()
            };

            let src_resource = upload_resource.clone();
            let src = D3D12_TEXTURE_COPY_LOCATION {
                pResource: Some(src_resource),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Footprint: self.layout.Footprint,
                        Offset: 0,
                    },
                },
            };

            resource_transition(
                command_list,
                texture_resource.clone(),
                D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                D3D12_RESOURCE_STATE_COPY_DEST,
            );

            unsafe {
                command_list.CopyTextureRegion(&dst, 0, 0, 0, &src, std::ptr::null());
            };

            resource_transition(
                command_list,
                texture_resource.clone(),
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
            );
        }

        unsafe {
            command_list.SetGraphicsRootDescriptorTable(1, self.resource_view.gpu_handle());
        }

        Ok(())
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}
