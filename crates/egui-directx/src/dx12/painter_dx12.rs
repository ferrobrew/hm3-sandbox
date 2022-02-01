use std::{
    collections::HashMap,
    ffi::CStr,
    ptr,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Context, Result};
use egui::{epaint::Vertex, vec2, ClippedMesh, FontImage, TextureId};
use windows::Win32::{
    Foundation::{BOOL, PSTR, RECT},
    Graphics::{
        Direct3D::{Fxc::D3DCompile2, ID3DBlob},
        Direct3D12::{
            D3D12SerializeRootSignature, ID3D12CommandQueue, ID3D12Device, ID3D12PipelineState,
            ID3D12RootSignature, D3D12_BLEND_DESC, D3D12_BLEND_INV_SRC_ALPHA, D3D12_BLEND_ONE,
            D3D12_BLEND_OP_ADD, D3D12_COLOR_WRITE_ENABLE_ALL, D3D12_COMPARISON_FUNC_ALWAYS,
            D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF, D3D12_CULL_MODE_NONE,
            D3D12_DEPTH_STENCIL_DESC, D3D12_DEPTH_WRITE_MASK_ZERO,
            D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            D3D12_DESCRIPTOR_RANGE, D3D12_DESCRIPTOR_RANGE_TYPE_SRV, D3D12_FILL_MODE_SOLID,
            D3D12_FILTER_MIN_MAG_MIP_LINEAR, D3D12_GRAPHICS_PIPELINE_STATE_DESC,
            D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, D3D12_INPUT_ELEMENT_DESC,
            D3D12_INPUT_LAYOUT_DESC, D3D12_LOGIC_OP_NOOP, D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            D3D12_RASTERIZER_DESC, D3D12_RENDER_TARGET_BLEND_DESC, D3D12_ROOT_PARAMETER,
            D3D12_ROOT_PARAMETER_TYPE_CBV, D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            D3D12_ROOT_SIGNATURE_DESC,
            D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT, D3D12_SHADER_BYTECODE,
            D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK, D3D12_STATIC_SAMPLER_DESC,
            D3D12_TEXTURE_ADDRESS_MODE_CLAMP, D3D_ROOT_SIGNATURE_VERSION_1,
        },
        Dxgi::{
            Common::{
                DXGI_FORMAT_R32G32_FLOAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_UNKNOWN,
                DXGI_SAMPLE_DESC,
            },
            IDXGISwapChain4,
        },
    },
};

use super::{
    super::painter::Painter, buffer::Buffer, descriptor_heap::DescriptorHeap,
    frame_context::FrameContext, texture::Texture,
};

#[repr(C)]
pub struct CBuffer {
    pub screen_size: [f32; 2],
}

pub struct PainterDX12 {
    device: ID3D12Device,
    command_queue: ID3D12CommandQueue,
    swap_chain: IDXGISwapChain4,
    root_signature: ID3D12RootSignature,
    pipeline_state: ID3D12PipelineState,
    frame_contexts: Vec<FrameContext>,
    rtv_heap: Arc<Mutex<DescriptorHeap>>,
    descriptor_heap: Arc<Mutex<DescriptorHeap>>,
    constant_buffer: Buffer<CBuffer>,
    vertex_buffer: Buffer<Vertex>,
    index_buffer: Buffer<u16>,
    texture: Option<Texture>,
    texture_version: Option<u64>,
    user_textures: HashMap<u64, Texture>,
    width: u32,
    height: u32,
}

fn create_root_signature(device: &ID3D12Device) -> Result<ID3D12RootSignature> {
    let mut root_parameters = [
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV,
            ..Default::default()
        },
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            ..Default::default()
        },
    ];

    let ranges = [D3D12_DESCRIPTOR_RANGE {
        RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
        NumDescriptors: 1,
        BaseShaderRegister: 2,
        ..Default::default()
    }];

    root_parameters[1]
        .Anonymous
        .DescriptorTable
        .pDescriptorRanges = ranges.as_ptr() as _;
    root_parameters[1]
        .Anonymous
        .DescriptorTable
        .NumDescriptorRanges = ranges.len() as _;

    let static_samplers = [D3D12_STATIC_SAMPLER_DESC {
        Filter: D3D12_FILTER_MIN_MAG_MIP_LINEAR,
        AddressU: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressV: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        ComparisonFunc: D3D12_COMPARISON_FUNC_ALWAYS,
        BorderColor: D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK,
        ShaderRegister: 1,
        ..Default::default()
    }];

    let root_signature_desc = D3D12_ROOT_SIGNATURE_DESC {
        NumParameters: root_parameters.len() as _,
        pParameters: root_parameters.as_ptr() as _,
        NumStaticSamplers: static_samplers.len() as _,
        pStaticSamplers: static_samplers.as_ptr() as _,
        Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
    };

    let mut blob = None;
    let mut error_blob = None;

    unsafe {
        if let Err(serialize_error) = D3D12SerializeRootSignature(
            &root_signature_desc,
            D3D_ROOT_SIGNATURE_VERSION_1,
            &mut blob,
            &mut error_blob,
        ) {
            if let Some(error) = error_blob {
                let error = CStr::from_ptr(error.GetBufferPointer() as _).to_str()?;

                return Err(anyhow!(
                    "Failed to serialize root signature ('{serialize_error}'): '{error}'"
                ));
            }

            return Err(anyhow!(
                "Failed to serialize root signature ('{serialize_error}')"
            ));
        };

        let blob = blob.unwrap();

        Ok(device.CreateRootSignature::<ID3D12RootSignature>(
            0,
            blob.GetBufferPointer(),
            blob.GetBufferSize(),
        )?)
    }
}

fn compile_shader(source: &[u8], shader_model: &'static str) -> Result<ID3DBlob> {
    let mut blob = None;
    let mut error_blob = None;

    unsafe {
        if let Err(compile_error) = D3DCompile2(
            source.as_ptr() as _,
            source.len(),
            None,
            ptr::null(),
            None,
            PSTR(b"main\0".as_ptr() as _),
            PSTR(shader_model.as_ptr() as _),
            0,
            0,
            0,
            ptr::null(),
            0,
            &mut blob,
            &mut error_blob,
        ) {
            if let Some(error) = error_blob {
                let error = CStr::from_ptr(error.GetBufferPointer() as _)
                    .to_str()
                    .unwrap();
                return Err(anyhow!("{compile_error}: '{error}'"));
            }

            return Err(anyhow!("{compile_error}"));
        }
    }

    blob.context("invalid blob")
}

fn create_pipeline_state(
    device: &ID3D12Device,
    root_signature: &ID3D12RootSignature,
) -> Result<ID3D12PipelineState> {
    let vertex_shader = compile_shader(
        include_bytes!("shaders/vertex_shader.hlsl").as_ref(),
        "vs_5_1",
    )
    .unwrap();

    let pixel_shader = compile_shader(
        include_bytes!("shaders/pixel_shader.hlsl").as_ref(),
        "ps_5_1",
    )
    .unwrap();

    let input_elements = [
        D3D12_INPUT_ELEMENT_DESC {
            SemanticName: PSTR(b"POSITION\0".as_ptr() as _),
            Format: DXGI_FORMAT_R32G32_FLOAT,
            InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
            ..Default::default()
        },
        D3D12_INPUT_ELEMENT_DESC {
            SemanticName: PSTR(b"TEXCOORD\0".as_ptr() as _),
            Format: DXGI_FORMAT_R32G32_FLOAT,
            AlignedByteOffset: 8,
            InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
            ..Default::default()
        },
        D3D12_INPUT_ELEMENT_DESC {
            SemanticName: PSTR(b"COLOR\0".as_ptr() as _),
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            AlignedByteOffset: 16,
            InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
            ..Default::default()
        },
    ];

    let state_desc = unsafe {
        D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: Some(root_signature.clone()),
            VS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: vertex_shader.GetBufferPointer(),
                BytecodeLength: vertex_shader.GetBufferSize(),
            },
            PS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: pixel_shader.GetBufferPointer(),
                BytecodeLength: pixel_shader.GetBufferSize(),
            },
            BlendState: D3D12_BLEND_DESC {
                RenderTarget: [
                    D3D12_RENDER_TARGET_BLEND_DESC {
                        BlendEnable: BOOL(1),
                        LogicOpEnable: BOOL(0),
                        SrcBlend: D3D12_BLEND_ONE,
                        DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
                        BlendOp: D3D12_BLEND_OP_ADD,
                        SrcBlendAlpha: D3D12_BLEND_ONE,
                        DestBlendAlpha: D3D12_BLEND_INV_SRC_ALPHA,
                        BlendOpAlpha: D3D12_BLEND_OP_ADD,
                        LogicOp: D3D12_LOGIC_OP_NOOP,
                        RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL as _,
                    },
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                ],
                ..Default::default()
            },
            SampleMask: !0u32,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                ForcedSampleCount: 1,
                ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
                ..Default::default()
            },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO,
                DepthFunc: D3D12_COMPARISON_FUNC_ALWAYS,
                ..Default::default()
            },
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: input_elements.as_ptr() as _,
                NumElements: input_elements.len() as _,
            },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            RTVFormats: [
                DXGI_FORMAT_R8G8B8A8_UNORM,
                DXGI_FORMAT_UNKNOWN,
                DXGI_FORMAT_UNKNOWN,
                DXGI_FORMAT_UNKNOWN,
                DXGI_FORMAT_UNKNOWN,
                DXGI_FORMAT_UNKNOWN,
                DXGI_FORMAT_UNKNOWN,
                DXGI_FORMAT_UNKNOWN,
            ],
            SampleDesc: DXGI_SAMPLE_DESC {
                Quality: 0,
                Count: 1,
            },
            ..Default::default()
        }
    };

    Ok(unsafe { device.CreateGraphicsPipelineState::<ID3D12PipelineState>(&state_desc)? })
}

fn create_buffers(device: &ID3D12Device) -> Result<(Buffer<CBuffer>, Buffer<Vertex>, Buffer<u16>)> {
    Ok((
        Buffer::new(device, 1)?,
        Buffer::new(device, 16384)?,
        Buffer::new(device, 8192)?,
    ))
}

impl PainterDX12 {
    pub fn new(
        device: ID3D12Device,
        command_queue: ID3D12CommandQueue,
        swap_chain: IDXGISwapChain4,
    ) -> Result<Self> {
        let root_signature = create_root_signature(&device)?;
        let pipeline_state = create_pipeline_state(&device, &root_signature)?;

        let swap_chain_desc = unsafe { swap_chain.GetDesc()? };
        let rtv_heap = Arc::new(Mutex::new(
            DescriptorHeap::new(
                &device,
                D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                swap_chain_desc.BufferCount,
            )
            .context("Failed to create RTV heap")?,
        ));
        let descriptor_heap = Arc::new(Mutex::new(
            DescriptorHeap::new(&device, D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 512)
                .context("Failed to create descriptor heap")?,
        ));
        let frame_contexts = FrameContext::new(
            &device,
            &pipeline_state,
            &swap_chain,
            &swap_chain_desc,
            &rtv_heap,
        )?;
        let (constant_buffer, vertex_buffer, index_buffer) = create_buffers(&device)?;

        let width = swap_chain_desc.BufferDesc.Width;
        let height = swap_chain_desc.BufferDesc.Height;

        Ok(Self {
            device,
            command_queue,
            swap_chain,
            frame_contexts,
            rtv_heap,
            descriptor_heap,
            root_signature,
            pipeline_state,
            constant_buffer,
            vertex_buffer,
            index_buffer,
            texture: None,
            texture_version: None,
            user_textures: Default::default(),
            width,
            height,
        })
    }
}

impl Painter for PainterDX12 {
    fn name(&self) -> &'static str {
        "egui-DX12"
    }

    fn set_texture(&mut self, tex_id: u64, image: epi::Image) {
        let mut texture = Texture::new(
            &self.device,
            Arc::clone(&self.descriptor_heap),
            image.size[0] as u32,
            image.size[1] as u32,
        )
        .expect("Failed to create texture");
        texture
            .update(image.pixels)
            .expect("Failed to update texture");

        self.user_textures.insert(tex_id, texture);
    }

    fn free_texture(&mut self, tex_id: u64) {
        self.user_textures.remove_entry(&tex_id);
    }

    fn debug_info(&self) -> String {
        todo!()
    }

    fn upload_egui_texture(&mut self, font_image: &FontImage) {
        let mut create_texture = true;

        if let Some(texture) = &self.texture {
            if texture.width() == font_image.width as _
                && texture.height() == font_image.height as _
            {
                create_texture = false;
            }
        }

        if create_texture {
            let width = font_image.width as u32;
            let height = font_image.height as u32;
            self.texture = Some(
                Texture::new(
                    &self.device,
                    Arc::clone(&self.descriptor_heap),
                    width,
                    height,
                )
                .expect("Failed to create egui texture"),
            );
        }

        let texture_version = Some(font_image.version);

        if self.texture_version != texture_version {
            if let Some(texture) = &mut self.texture {
                texture
                    .update(font_image.srgba_pixels(1.0).collect())
                    .expect("Failed to update pixels");
            }

            self.texture_version = texture_version;
        }
    }

    fn paint_meshes(
        &mut self,
        clipped_meshes: Vec<ClippedMesh>,
        pixels_per_point: f32,
    ) -> anyhow::Result<()> {
        // Not sure how to do this without inlining....
        let frame_context = {
            let frame_index = unsafe { self.swap_chain.GetCurrentBackBufferIndex() as usize };
            self.frame_contexts
                .get_mut(frame_index)
                .context("failed to get frame context")?
        };

        let screen_size_pixels = vec2(self.width as f32, self.height as f32);
        let command_queue = &self.command_queue;
        let command_list = frame_context.command_list().clone();

        frame_context.begin_frame(
            screen_size_pixels,
            pixels_per_point,
            &self.root_signature,
            &self.descriptor_heap,
            &self.constant_buffer,
        )?;

        for ClippedMesh(clip_rect, mesh) in clipped_meshes {
            // Not sure how to do this without inlining....
            if let Some(texture) = {
                let texture_id = mesh.texture_id;
                match texture_id {
                    TextureId::Egui => self.texture.as_mut(),
                    TextureId::User(id) => self.user_textures.get_mut(&id),
                }
            } {
                // Transform clip rect to physical pixels:
                let clip_min_x = clip_rect.min.x * pixels_per_point;
                let clip_min_y = clip_rect.min.y * pixels_per_point;
                let clip_max_x = clip_rect.max.x * pixels_per_point;
                let clip_max_y = clip_rect.max.y * pixels_per_point;

                // Make sure clip rect can fit within a `i32`:
                let clip_min_x = clip_min_x.clamp(0.0, screen_size_pixels.x);
                let clip_min_y = clip_min_y.clamp(0.0, screen_size_pixels.y);
                let clip_max_x = clip_max_x.clamp(clip_min_x, screen_size_pixels.x);
                let clip_max_y = clip_max_y.clamp(clip_min_y, screen_size_pixels.y);

                let clip_min_x = clip_min_x.round() as i32;
                let clip_min_y = clip_min_y.round() as i32;
                let clip_max_x = clip_max_x.round() as i32;
                let clip_max_y = clip_max_y.round() as i32;

                // scissor Y coordinate is from the bottom
                unsafe {
                    command_list.RSSetScissorRects(
                        1,
                        &RECT {
                            left: clip_min_x,
                            top: clip_min_y,
                            right: clip_max_x,
                            bottom: clip_max_y,
                        },
                    );
                };

                let mut index_offset = 0;
                let mut vertex_offset = 0;

                texture.bind(&command_list)?;

                for mesh in mesh.split_to_u16() {
                    frame_context.draw_meshlet(
                        &mesh,
                        &mut index_offset,
                        &mut vertex_offset,
                        &self.index_buffer,
                        &self.vertex_buffer,
                    );
                }
            }
        }

        frame_context.end_frame(command_queue, &self.pipeline_state)?;

        Ok(())
    }

    fn resize_buffers<F, R>(&mut self, callback: F) -> Result<R>
    where
        F: FnOnce() -> R,
    {
        self.frame_contexts.clear();
        let result = callback();
        let swap_chain_desc = unsafe {
            self.swap_chain
                .GetDesc()
                .context("Failed to get swap chain description")?
        };
        self.width = swap_chain_desc.BufferDesc.Width;
        self.height = swap_chain_desc.BufferDesc.Height;
        self.frame_contexts = FrameContext::new(
            &self.device,
            &self.pipeline_state,
            &self.swap_chain,
            &swap_chain_desc,
            &self.rtv_heap,
        )?;
        Ok(result)
    }
}

unsafe impl Send for PainterDX12 {}
unsafe impl Sync for PainterDX12 {}

impl Drop for PainterDX12 {
    fn drop(&mut self) {
        self.frame_contexts.clear()
    }
}
