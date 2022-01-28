use std::mem;

use anyhow::{anyhow, Context, Result};

use windows::Win32::Graphics::Direct3D12::{
    ID3D12DescriptorHeap, ID3D12Device, D3D12_CPU_DESCRIPTOR_HANDLE, D3D12_DESCRIPTOR_HEAP_DESC,
    D3D12_DESCRIPTOR_HEAP_FLAG_NONE, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
    D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, D3D12_GPU_DESCRIPTOR_HANDLE,
};

pub struct DescriptorHandle {
    cpu_handle: D3D12_CPU_DESCRIPTOR_HANDLE,
    gpu_handle: D3D12_GPU_DESCRIPTOR_HANDLE,
}

impl DescriptorHandle {
    pub fn cpu_handle(&self) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        self.cpu_handle
    }

    pub fn gpu_handle(&self) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        self.gpu_handle
    }
}

pub struct DescriptorHeap {
    heap: ID3D12DescriptorHeap,
    heap_handle_cpu: D3D12_CPU_DESCRIPTOR_HANDLE,
    heap_handle_gpu: D3D12_GPU_DESCRIPTOR_HANDLE,
    heap_stride: u32,
    heap_allocations: Box<[usize]>,
}

const BLOCK_SIZE: usize = mem::size_of::<usize>() * 8;

impl DescriptorHeap {
    pub fn new(
        device: &ID3D12Device,
        heap_type: i32,
        heap_capacity: u32,
    ) -> Result<DescriptorHeap> {
        let heap_stride = unsafe { device.GetDescriptorHandleIncrementSize(heap_type) };

        // Calculate a heap capacity that allows us to have no left over bits
        let heap_capacity = {
            // Heap capacity to i64
            let capacity = heap_capacity as i64;
            // Calculate the step
            let step = BLOCK_SIZE as i64;
            // Round up to the nearest step in bytes (powers of two only)
            ((capacity + step - 1) & -step) as u32
        };

        let desc = D3D12_DESCRIPTOR_HEAP_DESC {
            Type: heap_type,
            NumDescriptors: heap_capacity,
            Flags: match heap_type {
                D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV => D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                _ => D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
            },
            ..Default::default()
        };
        let heap = unsafe {
            device
                .CreateDescriptorHeap::<ID3D12DescriptorHeap>(&desc)
                .context("Failed to create descriptor heap")?
        };
        let heap_handle_cpu = unsafe { heap.GetCPUDescriptorHandleForHeapStart() };
        let heap_handle_gpu = unsafe { heap.GetGPUDescriptorHandleForHeapStart() };
        let heap_allocations_size = heap_capacity as usize / BLOCK_SIZE;

        Ok(DescriptorHeap {
            heap,
            heap_handle_cpu,
            heap_handle_gpu,
            heap_stride,
            heap_allocations: vec![0; heap_allocations_size].into_boxed_slice(),
        })
    }

    pub fn heap(&self) -> &ID3D12DescriptorHeap {
        &self.heap
    }

    pub fn allocate(&mut self) -> Result<DescriptorHandle> {
        if let Some(chunk_index) = self
            .heap_allocations
            .iter()
            .position(|chunk| chunk.count_zeros() != 0)
        {
            let chunk_value = &mut self.heap_allocations[chunk_index];
            // Bitwise not the value so allocations are 0 and free spaces are 1
            let chunk_bit_mask = !*chunk_value as isize;
            // Find the first unused bit in this chunk
            let chunk_bit_mask = (chunk_bit_mask & -chunk_bit_mask) as usize;
            // Calculate leading zeros
            let leading_zeros = chunk_bit_mask.leading_zeros() as usize - 1;
            // Calculate log2 of the mask
            let chunk_bit_index = BLOCK_SIZE - leading_zeros - 2;
            // Mark the allocation bit
            *chunk_value ^= 1 << chunk_bit_index;
            // Calculate final offset
            let offset = chunk_index * BLOCK_SIZE + chunk_bit_index;

            Ok(DescriptorHandle {
                cpu_handle: D3D12_CPU_DESCRIPTOR_HANDLE {
                    ptr: self.heap_handle_cpu.ptr + offset * self.heap_stride as usize,
                },
                gpu_handle: D3D12_GPU_DESCRIPTOR_HANDLE {
                    ptr: self.heap_handle_gpu.ptr + offset as u64 * self.heap_stride as u64,
                },
            })
        } else {
            return Err(anyhow!(
                "Failed to create descriptor heap allocation: no space free"
            ));
        }
    }

    pub fn free(&mut self, handle: DescriptorHandle) -> Result<()> {
        let index = (handle.cpu_handle.ptr - self.heap_handle_cpu.ptr) as u32 / self.heap_stride;
        let chunk_index = index as usize / BLOCK_SIZE;
        let chunk_bit_index = index as usize % BLOCK_SIZE;
        let chunk_bit_mask = (1 << chunk_bit_index) as usize;

        if let Some(chunk) = self.heap_allocations.get_mut(chunk_index) {
            if *chunk & chunk_bit_mask != 0 {
                *chunk ^= chunk_bit_mask;
            } else {
                return Err(anyhow!(
                    "Failed to free allocation: allocation already freed"
                ));
            }
        } else {
            return Err(anyhow!(
                "Failed to free descriptor heap allocation: invalid descriptor handle"
            ));
        }

        Ok(())
    }
}
