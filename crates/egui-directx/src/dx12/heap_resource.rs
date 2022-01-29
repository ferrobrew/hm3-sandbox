use std::sync::{Arc, Mutex};

use windows::Win32::Graphics::Direct3D12::{
    D3D12_CPU_DESCRIPTOR_HANDLE, D3D12_GPU_DESCRIPTOR_HANDLE,
};

use super::descriptor_heap::{DescriptorHandle, DescriptorHeap};

pub struct HeapResource {
    heap: Arc<Mutex<DescriptorHeap>>,
    handle: Option<DescriptorHandle>,
}

impl HeapResource {
    pub fn new(heap: Arc<Mutex<DescriptorHeap>>, handle: DescriptorHandle) -> Self {
        Self {
            heap,
            handle: Some(handle),
        }
    }

    pub fn cpu_handle(&self) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        let handle = self
            .handle
            .as_ref()
            .expect("Failed to get heap resouce handle");
        handle.cpu_handle()
    }

    pub fn gpu_handle(&self) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        let handle = self
            .handle
            .as_ref()
            .expect("Failed to get heap resouce handle");
        handle.gpu_handle()
    }
}

impl Drop for HeapResource {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let mut heap = self.heap.lock().expect("Failed to get lock on heap");
            heap.free(handle).expect("Failed to free heap resource");
        }
    }
}
