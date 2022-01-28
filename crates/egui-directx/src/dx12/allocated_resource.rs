use std::sync::{Arc, Mutex};

use gpu_allocator::d3d12::{Allocation, Allocator};
use windows::Win32::Graphics::Direct3D12::ID3D12Resource;

pub struct AllocatedResource {
    allocator: Arc<Mutex<Allocator>>,
    allocation: Option<Allocation>,
    handle: ID3D12Resource,
}

impl AllocatedResource {
    pub fn new(
        allocator: Arc<Mutex<Allocator>>,
        allocation: Allocation,
        handle: ID3D12Resource,
    ) -> Self {
        Self {
            allocator,
            allocation: Some(allocation),
            handle,
        }
    }

    pub fn handle(&self) -> ID3D12Resource {
        self.handle.clone()
    }
}

impl Drop for AllocatedResource {
    fn drop(&mut self) {
        if let Some(allocation) = self.allocation.take() {
            let mut allocator = self.allocator
                .lock()
                .expect("Failed to get lock on allocator");
            allocator.free(allocation)
                .expect("Failed to free allocated resource");
        }
    }
}
