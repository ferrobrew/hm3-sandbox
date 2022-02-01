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
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        Graphics::{
            Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
            Direct3D12::{
                ID3D12CommandAllocator, ID3D12CommandList, ID3D12CommandQueue, ID3D12Device,
                ID3D12Fence, ID3D12GraphicsCommandList, ID3D12PipelineState, ID3D12Resource,
                ID3D12RootSignature, D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_FENCE_FLAG_NONE,
                D3D12_INDEX_BUFFER_VIEW, D3D12_RESOURCE_BARRIER, D3D12_RESOURCE_BARRIER_0,
                D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_BARRIER_FLAG_NONE,
                D3D12_RESOURCE_BARRIER_TYPE_TRANSITION, D3D12_RESOURCE_STATES,
                D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_TRANSITION_BARRIER, D3D12_VERTEX_BUFFER_VIEW, D3D12_VIEWPORT,
            },
            Dxgi::{Common::DXGI_FORMAT_R16_UINT, IDXGISwapChain4, DXGI_SWAP_CHAIN_DESC},
        },
        System::Threading::{CreateEventA, WaitForSingleObject},
    },
};

use super::{
    back_buffer::BackBuffer, buffer::Buffer, descriptor_heap::DescriptorHeap,
    heap_resource::HeapResource, painter_dx12::CBuffer,
};

pub struct FrameContext {
    fence: ID3D12Fence,
    fence_value: u64,
    fence_event: HANDLE,
    back_buffer: BackBuffer,
    command_allocator: ID3D12CommandAllocator,
    command_list: ID3D12GraphicsCommandList,
}

pub fn resource_transition(
    command_list: &ID3D12GraphicsCommandList,
    resource: ID3D12Resource,
    state_before: D3D12_RESOURCE_STATES,
    state_after: D3D12_RESOURCE_STATES,
) {
    let resource = Some(resource);
    let barrier = D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: resource,
                StateBefore: state_before,
                StateAfter: state_after,
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            }),
        },
    };
    unsafe {
        command_list.ResourceBarrier(1, &barrier);
        mem::ManuallyDrop::into_inner(barrier.Anonymous.Transition);
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
                let fence = device
                    .CreateFence::<ID3D12Fence>(0, D3D12_FENCE_FLAG_NONE)
                    .context("Failed to create fence")?;
                let fence_event = CreateEventA(ptr::null_mut(), false, false, None);
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
                    .allocate()
                    .context("Failed to allocate rtv heap resource")?;
                let descriptor_handle = HeapResource::new(Arc::clone(rtv_heap), handle);
                let back_buffer = BackBuffer::new(device, resource, format, descriptor_handle);

                frame_contexts.push(FrameContext {
                    fence,
                    fence_value: 1,
                    fence_event,
                    back_buffer,
                    command_allocator,
                    command_list,
                });
            }
        }

        Ok(frame_contexts)
    }

    pub fn command_list(&self) -> &ID3D12GraphicsCommandList {
        &self.command_list
    }

    pub fn sync(&mut self, command_queue: &ID3D12CommandQueue) -> Result<()> {
        unsafe {
            command_queue
                .Signal(&self.fence, self.fence_value)
                .context("Failed to signal fence")?;
            self.fence
                .SetEventOnCompletion(self.fence_value, self.fence_event)
                .context("Failed to set fence event")?;
            self.fence_value += 1;
            WaitForSingleObject(self.fence_event, u32::MAX);
        }

        Ok(())
    }

    fn reset_command_list(&mut self, pipeline_state: &ID3D12PipelineState) -> Result<()> {
        unsafe {
            self.command_allocator
                .Reset()
                .context("Failed to resent command allocator")?;
            self.command_list
                .Reset(&self.command_allocator, pipeline_state)
                .context("Failed to reset command list")?;
        }

        Ok(())
    }

    pub fn begin_frame(
        &mut self,
        screen_size_pixels: Vec2,
        //pixels_per_point: f32,
        root_signature: &ID3D12RootSignature,
        descriptor_heap: &Arc<Mutex<DescriptorHeap>>,
        constant_buffer: &Buffer<CBuffer>,
    ) -> Result<()> {
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
        let screen_size = screen_size_pixels;// / pixels_per_point;

        unsafe {
            self.command_list.OMSetBlendFactor(blend_factor.as_ptr());
            self.command_list.SetGraphicsRootSignature(root_signature);
            self.command_list
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

            // Bind and upload constant buffer
            ptr::copy_nonoverlapping(
                &CBuffer {
                    screen_size: screen_size.into(),
                },
                constant_buffer.get_ptr(0),
                1,
            );
            self.command_list.SetGraphicsRootConstantBufferView(
                0,
                constant_buffer.handle().GetGPUVirtualAddress(),
            );
        }

        // Set up texture descriptor heap
        unsafe {
            let heap = descriptor_heap
                .lock()
                .expect("Failed to get descriptor heap lock")
                .heap()
                .clone();
            self.command_list.SetDescriptorHeaps(1, &Some(heap));
        }

        // Setup frame buffer as a render target
        resource_transition(
            &self.command_list,
            self.back_buffer.resource().clone(),
            D3D12_RESOURCE_STATE_PRESENT,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        );

        unsafe {
            self.command_list
                .OMSetRenderTargets(1, &self.back_buffer.handle(), false, ptr::null());
        }

        Ok(())
    }

    pub fn draw_meshlet(
        &self,
        mesh: &Mesh16,
        index_offset: &mut usize,
        vertex_offset: &mut usize,
        index_buffer: &Buffer<u16>,
        vertex_buffer: &Buffer<Vertex>,
    ) {
        // Bind and upload index buffer
        let stride = mem::size_of::<u16>();
        let size = mesh.indices.len() * stride;
        let handle = &index_buffer.handle();
        unsafe {
            ptr::copy_nonoverlapping(
                mesh.indices.as_ptr(),
                index_buffer.get_ptr(*index_offset),
                mesh.indices.len(),
            );
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
        let handle = &vertex_buffer.handle();
        unsafe {
            ptr::copy_nonoverlapping(
                mesh.vertices.as_ptr(),
                vertex_buffer.get_ptr(*vertex_offset),
                mesh.vertices.len(),
            );
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

        *index_offset += mesh.indices.len();
        *vertex_offset += mesh.vertices.len();
    }

    pub fn end_frame(
        &mut self,
        command_queue: &ID3D12CommandQueue,
        pipeline_state: &ID3D12PipelineState,
    ) -> Result<()> {
        resource_transition(
            &self.command_list,
            self.back_buffer.resource().clone(),
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_PRESENT,
        );

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

impl Drop for FrameContext {
    fn drop(&mut self) {
        unsafe {
            self.command_list
                .Close()
                .expect("Failed to close command list");
            self.command_allocator
                .Reset()
                .expect("Failed to release command allocator");
            CloseHandle(self.fence_event);
        }
    }
}
