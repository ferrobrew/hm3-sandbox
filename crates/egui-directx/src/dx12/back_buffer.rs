use windows::Win32::Graphics::Direct3D12::{
    ID3D12Device, ID3D12Resource, D3D12_CPU_DESCRIPTOR_HANDLE, D3D12_RENDER_TARGET_VIEW_DESC,
    D3D12_RTV_DIMENSION_TEXTURE2D,
};

use super::heap_resource::HeapResource;

pub struct BackBuffer {
    resource: ID3D12Resource,
    heap_resource: HeapResource,
}

impl BackBuffer {
    pub fn new(
        device: &ID3D12Device,
        resource: ID3D12Resource,
        format: u32,
        heap_resource: HeapResource,
    ) -> Self {
        unsafe {
            device.CreateRenderTargetView(
                &resource,
                &D3D12_RENDER_TARGET_VIEW_DESC {
                    Format: format,
                    ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D,
                    ..Default::default()
                },
                heap_resource.cpu_handle(),
            );
        }

        Self {
            resource,
            heap_resource,
        }
    }

    pub fn resource(&self) -> &ID3D12Resource {
        &self.resource
    }

    pub fn handle(&self) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        self.heap_resource.cpu_handle()
    }
}
