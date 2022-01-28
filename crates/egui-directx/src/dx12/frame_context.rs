use std::{
    mem, ptr,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use egui::{
    epaint::{Mesh16, Vertex},
    Vec2,
};
use windows::{
    core::Interface,
    Win32::Graphics::{
        Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        Direct3D12::{
            ID3D12CommandAllocator, ID3D12CommandList, ID3D12CommandQueue, ID3D12DescriptorHeap,
            ID3D12Device, ID3D12Fence, ID3D12GraphicsCommandList, ID3D12PipelineState,
            ID3D12Resource, ID3D12RootSignature, D3D12_COMMAND_LIST_TYPE_DIRECT,
            D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            D3D12_FENCE_FLAG_NONE, D3D12_GPU_DESCRIPTOR_HANDLE, D3D12_INDEX_BUFFER_VIEW,
            D3D12_RESOURCE_BARRIER, D3D12_RESOURCE_BARRIER_0,
            D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_BARRIER_FLAG_NONE,
            D3D12_RESOURCE_BARRIER_TYPE_TRANSITION, D3D12_RESOURCE_STATES,
            D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_TRANSITION_BARRIER, D3D12_VERTEX_BUFFER_VIEW, D3D12_VIEWPORT,
        },
        Dxgi::{Common::DXGI_FORMAT_R16_UINT, IDXGISwapChain4, DXGI_SWAP_CHAIN_DESC},
    },
};

use super::{
    back_buffer::BackBuffer, buffer::Buffer, descriptor_heap::DescriptorHeap,
    heap_resource::HeapResource, painter_dx12::CBuffer, texture::Texture,
};

pub struct FrameContext {
    fence_value: u64,
    fence: ID3D12Fence,
    back_buffer: BackBuffer,
    command_allocator: ID3D12CommandAllocator,
    command_list: ID3D12GraphicsCommandList,
}

pub fn resource_transition(
    resource: &ID3D12Resource,
    state_before: D3D12_RESOURCE_STATES,
    state_after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: Some(resource.clone()),
                StateBefore: state_before,
                StateAfter: state_after,
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            }),
        },
    }
}

impl FrameContext {
    pub fn new(
        device: &ID3D12Device,
        pipeline_state: &ID3D12PipelineState,
        swap_chain: &IDXGISwapChain4,
        swap_chain_desc: &DXGI_SWAP_CHAIN_DESC,
        rtv_heap: &Arc<Mutex<DescriptorHeap>>,
    ) -> Result<Vec<FrameContext>> {
        let capacity = swap_chain_desc.BufferCount as _;
        let mut frame_contexts = Vec::with_capacity(capacity);
        let format = swap_chain_desc.BufferDesc.Format;

        for i in 0..capacity {
            unsafe {
                let fence_value = 0;
                let fence = device
                    .CreateFence::<ID3D12Fence>(fence_value, D3D12_FENCE_FLAG_NONE)
                    .context("Failed to create fence")?;
                let command_allocator = device
                    .CreateCommandAllocator::<ID3D12CommandAllocator>(
                        D3D12_COMMAND_LIST_TYPE_DIRECT,
                    )
                    .context("Failed to create command allocator")?;
                let command_list = device
                    .CreateCommandList(
                        0,
                        D3D12_COMMAND_LIST_TYPE_DIRECT,
                        &command_allocator,
                        pipeline_state,
                    )
                    .context("Failed to create command list")?;
                let resource = swap_chain
                    .GetBuffer::<ID3D12Resource>(i as _)
                    .context("Failed to get buffer resource")?;

                let handle = rtv_heap
                    .lock()
                    .expect("Failed to get heap lock")
                    .allocate()?;
                let descriptor_handle = HeapResource::new(Arc::clone(rtv_heap), handle);
                let back_buffer = BackBuffer::new(device, resource, format, descriptor_handle);

                frame_contexts.push(FrameContext {
                    fence_value,
                    fence,
                    back_buffer,
                    command_allocator,
                    command_list,
                });
            }
        }

        Ok(frame_contexts)
    }

    pub fn command_list(&self) -> ID3D12GraphicsCommandList {
        self.command_list.clone()
    }

    pub fn sync(&mut self, command_queue: &ID3D12CommandQueue) -> Result<()> {
        unsafe {
            self.fence_value += 1;
            command_queue
                .Signal(&self.fence, self.fence_value)
                .context("Failed to signal fence")?;

            loop {
                if self.fence_value == self.fence.GetCompletedValue() {
                    break;
                }
            }
        }

        Ok(())
    }

    fn reset_command_list(&mut self, pipeline_state: &ID3D12PipelineState) -> Result<()> {
        unsafe {
            self.command_list
                .Reset(&self.command_allocator, pipeline_state)
                .context("Failed to reset command list")?;
        }

        Ok(())
    }

    pub fn begin_frame(
        &mut self,
        screen_size_pixels: &Vec2,
        pixels_per_point: f32,
        root_signature: &ID3D12RootSignature,
        descriptor_heap: &Arc<Mutex<DescriptorHeap>>,
        constant_buffer: &Buffer<CBuffer>,
        command_queue: &ID3D12CommandQueue,
    ) -> Result<()> {
        self.sync(command_queue)?;

        // Set viewport
        let viewport = D3D12_VIEWPORT {
            Width: screen_size_pixels.x,
            Height: screen_size_pixels.y,
            MaxDepth: 1.0,
            ..Default::default()
        };

        unsafe {
            self.command_list.RSSetViewports(1, &viewport);
        }

        // Miscellaneous render state
        let blend_factor = [0f32; 4];
        let screen_size_points = *screen_size_pixels / pixels_per_point;

        unsafe {
            self.command_list.OMSetBlendFactor(blend_factor.as_ptr());
            self.command_list.SetGraphicsRootSignature(root_signature);
            self.command_list
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

            // Bind and upload constant buffer
            ptr::copy_nonoverlapping(
                &CBuffer {
                    screen_size_points: screen_size_points.into(),
                },
                constant_buffer.pointer(),
                1,
            );
            self.command_list.SetGraphicsRootConstantBufferView(
                0,
                constant_buffer.resource_handle().GetGPUVirtualAddress(),
            );
        }

        // Set up texture descriptor heap
        let heap = Some(descriptor_heap.lock().expect("Failed to get descriptor heap lock").heap());

        unsafe {
            self.command_list
                .SetDescriptorHeaps(1, mem::transmute(&heap));
        }

        // Setup frame buffer as a render target
        let barrier = resource_transition(
            self.back_buffer.resource(),
            D3D12_RESOURCE_STATE_PRESENT,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        );

        unsafe {
            self.command_list.ResourceBarrier(1, &barrier);
            self.command_list
                .OMSetRenderTargets(1, &self.back_buffer.handle(), false, ptr::null())
        }

        self.sync(command_queue)?;

        Ok(())
    }

    pub fn draw_meshlet(
        &self,
        mesh: &Mesh16,
        index_offset: &mut u32,
        vertex_offset: &mut u32,
        index_buffer: &Buffer<u16>,
        vertex_buffer: &Buffer<Vertex>,
    ) {
        // Bind and upload index buffer
        let stride = mem::size_of::<u16>();
        let size = mesh.indices.len() * stride;
        let handle = &index_buffer.resource_handle();
        unsafe {
            let pointer = index_buffer.pointer().add(*index_offset as _);
            ptr::copy_nonoverlapping(mesh.indices.as_ptr(), pointer, mesh.indices.len());
            self.command_list
                .IASetIndexBuffer(&D3D12_INDEX_BUFFER_VIEW {
                    BufferLocation: handle.GetGPUVirtualAddress() + *index_offset as u64,
                    SizeInBytes: size as _,
                    Format: DXGI_FORMAT_R16_UINT,
                });
        }

        // Bind and upload vertex buffer
        let stride = mem::size_of::<Vertex>();
        let size = mesh.vertices.len() * stride;
        let handle = &vertex_buffer.resource_handle();
        unsafe {
            let pointer = vertex_buffer.pointer().add(*vertex_offset as _);
            ptr::copy_nonoverlapping(mesh.vertices.as_ptr(), pointer, mesh.vertices.len());
            self.command_list.IASetVertexBuffers(
                0,
                1,
                &D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: handle.GetGPUVirtualAddress() + *vertex_offset as u64,
                    SizeInBytes: size as _,
                    StrideInBytes: stride as _,
                },
            );
        };

        // Draw instance
        unsafe {
            self.command_list.DrawIndexedInstanced(
                mesh.indices.len() as _,
                1,
                *index_offset as _,
                *vertex_offset as _,
                0,
            );
        }

        *index_offset += mesh.indices.len() as u32;
        *vertex_offset += mesh.vertices.len() as u32;
    }

    pub fn end_frame(
        &mut self,
        command_queue: &ID3D12CommandQueue,
        pipeline_state: &ID3D12PipelineState,
    ) -> Result<()> {
        let barrier = resource_transition(
            self.back_buffer.resource(),
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_PRESENT,
        );

        unsafe {
            self.command_list.ResourceBarrier(1, &barrier);
        }

        unsafe {
            self.command_list
                .Close()
                .context("Failed to close command list")?;
            let command_list = self.command_list.cast::<ID3D12CommandList>().ok();
            command_queue.ExecuteCommandLists(1, &command_list);
        }

        self.sync(command_queue)?;
        self.reset_command_list(pipeline_state)?;

        Ok(())
    }
}
