#![allow(non_snake_case)]

use std::{mem, ptr, slice, cmp};

use winapi::Interface as _;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::shared::winerror::*;
use winapi::shared::dxgi::*;
use winapi::shared::dxgi1_2::*;
use winapi::shared::dxgi1_3::*;
use winapi::shared::dxgitype::*;
use winapi::shared::dxgiformat::*;
use winapi::um::winbase::*;
use winapi::um::winnt::*;
use winapi::um::winuser::*;
use winapi::um::synchapi::*;
use winapi::um::d3dcommon::*;
use winapi::um::d3d11::*;

use win32::{HResult, Com};

use crate::Context;

pub struct Draw {
    device: Com<ID3D11Device>,
    context: Com<ID3D11DeviceContext>,
    swap_chain: Com<IDXGISwapChain2>,
    frame_wait: HANDLE,
    rtv: Option<Com<ID3D11RenderTargetView>>,
    rtv_size: (LONG, LONG),

    input_layout: Com<ID3D11InputLayout>,
    sampler: Com<ID3D11SamplerState>,
    vertex_shader: Com<ID3D11VertexShader>,
    rs: Com<ID3D11RasterizerState>,
    pixel_shader: Com<ID3D11PixelShader>,
    dss: Com<ID3D11DepthStencilState>,
    bs: Com<ID3D11BlendState>,

    view: Com<ID3D11Buffer>,
    material: Com<ID3D11Buffer>,
    srv: Com<ID3D11ShaderResourceView>,
    vertex_buffer: Com<ID3D11Buffer>,
    index_buffer: Com<ID3D11Buffer>,
    vertex_capacity: UINT,
    index_capacity: UINT,
}

#[allow(dead_code)]
#[repr(align(16))]
struct View {
    view_size: [f32; 2],
    port_size: [f32; 2],
}

#[allow(dead_code)]
#[repr(align(16))]
struct Material {
    atlas_size: [f32; 2],
}

pub fn load(cx: &mut Context) { unsafe {
    let Context { world, assets, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { platform, graphics, .. } = draw;
    let &mut crate::platform::Draw { hwnd, .. } = platform;

    // device

    let mut flags = 0;
    if cfg!(debug_assertions) {
        flags |= D3D11_CREATE_DEVICE_DEBUG;
    }

    let mut device = ptr::null_mut();
    let mut context = ptr::null_mut();
    match D3D11CreateDevice(
        ptr::null_mut(),
        D3D_DRIVER_TYPE_HARDWARE, ptr::null_mut(),
        flags,
        ptr::null_mut(), 0,
        D3D11_SDK_VERSION,
        &mut device, ptr::null_mut(), &mut context
    ) {
        hr if FAILED(hr) => panic!("failed to create device: {}", HResult(hr)),
        _ => (),
    }
    let device = Com::from_raw(device);
    let context = Com::from_raw(context);

    let dxgi_device = device.query_interface::<IDXGIDevice>()
        .unwrap_or_else(|hr| panic!("failed to get dxgi device: {}", HResult(hr)));

    let mut dxgi_adapter = ptr::null_mut();
    match dxgi_device.GetAdapter(&mut dxgi_adapter) {
        hr if FAILED(hr) => panic!("failed to get dxgi adapter: {}", HResult(hr)),
        _ => (),
    }
    let dxgi_adapter = Com::from_raw(dxgi_adapter);

    let mut dxgi_factory = ptr::null_mut();
    match dxgi_adapter.GetParent(&IDXGIFactory2::uuidof(), &mut dxgi_factory) {
        hr if FAILED(hr) => panic!("failed to get dxgi factory: {}", HResult(hr)),
        _ => (),
    }
    let dxgi_factory = Com::from_raw(dxgi_factory as *mut IDXGIFactory2);

    let scd = DXGI_SWAP_CHAIN_DESC1 {
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            ..mem::zeroed()
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: DXGI_SCALING_NONE,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
        ..mem::zeroed()
    };

    let mut swap_chain = ptr::null_mut();
    match dxgi_factory.CreateSwapChainForHwnd(
        &**device as *const _ as *mut _, hwnd, &scd, ptr::null_mut(), ptr::null_mut(),
        &mut swap_chain
    ) {
        hr if FAILED(hr) => panic!("failed to create swap chain: {}", HResult(hr)),
        _ => (),
    }
    let swap_chain = Com::from_raw(swap_chain);
    let swap_chain = swap_chain.query_interface::<IDXGISwapChain2>()
        .unwrap_or_else(|hr| panic!("failed to downcast swap chain: {}", HResult(hr)));

    let frame_wait = swap_chain.GetFrameLatencyWaitableObject();

    let rtv = Some(create_rtv(&device, &swap_chain));

    let mut rect = RECT { ..mem::zeroed() };
    GetClientRect(hwnd, &mut rect);
    let rtv_size = (rect.right, rect.bottom);

    // pipeline

    let ied = [
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: b"POSITION\0".as_ptr() as *const i8,
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32B32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: D3D11_APPEND_ALIGNED_ELEMENT,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: b"TEXCOORD\0".as_ptr() as *const i8,
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: D3D11_APPEND_ALIGNED_ELEMENT,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: b"TEXCOORD\0".as_ptr() as *const i8,
            SemanticIndex: 1,
            Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: D3D11_APPEND_ALIGNED_ELEMENT,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
    ];

    let vertex_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/vertex.cso"));

    let mut input_layout = ptr::null_mut();
    match device.CreateInputLayout(
        ied.as_ptr(), ied.len() as UINT,
        vertex_bytes.as_ptr() as *const _, vertex_bytes.len(),
        &mut input_layout
    ) {
        hr if FAILED(hr) => panic!("failed to create input layout: {}", HResult(hr)),
        _ => (),
    }
    let input_layout = Com::from_raw(input_layout);

    let mut sampler = ptr::null_mut();
    let sd = D3D11_SAMPLER_DESC {
        Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
        AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
        MipLODBias: 0.0,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D11_COMPARISON_NEVER,
        BorderColor: [1.0, 1.0, 1.0, 1.0],
        MinLOD: -D3D11_FLOAT32_MAX,
        MaxLOD: D3D11_FLOAT32_MAX,
    };
    match device.CreateSamplerState(&sd, &mut sampler) {
        hr if FAILED(hr) => panic!("failed to create sampler: {}", HResult(hr)),
        _ => (),
    }
    let sampler = Com::from_raw(sampler);

    let mut vertex_shader = ptr::null_mut();
    match device.CreateVertexShader(
        vertex_bytes.as_ptr() as *const _, vertex_bytes.len(), ptr::null_mut(), &mut vertex_shader
    ) {
        hr if FAILED(hr) => panic!("failed to create vertex shader: {}", HResult(hr)),
        _ => (),
    }
    let vertex_shader = Com::from_raw(vertex_shader);

    let rsd = D3D11_RASTERIZER_DESC {
        FillMode: D3D11_FILL_SOLID,
        CullMode: D3D11_CULL_BACK,
        FrontCounterClockwise: TRUE,
        DepthBias: 0,
        DepthBiasClamp: 0.0,
        SlopeScaledDepthBias: 0.0,
        DepthClipEnable: TRUE,
        ScissorEnable: FALSE,
        MultisampleEnable: FALSE,
        AntialiasedLineEnable: FALSE,
        ..mem::zeroed()
    };

    let mut rs = ptr::null_mut();
    match device.CreateRasterizerState(&rsd, &mut rs) {
        hr if FAILED(hr) => panic!("failed to create rasterizer state: {}", HResult(hr)),
        _ => (),
    }
    let rs = Com::from_raw(rs);

    let pixel_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/pixel.cso"));

    let mut pixel_shader = ptr::null_mut();
    match device.CreatePixelShader(
        pixel_bytes.as_ptr() as *const _, pixel_bytes.len(), ptr::null_mut(), &mut pixel_shader
    ) {
        hr if FAILED(hr) => panic!("failed to create pixel shader: {}", HResult(hr)),
        _ => (),
    }
    let pixel_shader = Com::from_raw(pixel_shader);

    let dsd = D3D11_DEPTH_STENCIL_DESC {
        DepthEnable: FALSE,
        DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ZERO,
        DepthFunc: D3D11_COMPARISON_ALWAYS,
        StencilEnable: FALSE,
        ..mem::zeroed()
    };

    let mut dss = ptr::null_mut();
    match device.CreateDepthStencilState(&dsd, &mut dss) {
        hr if FAILED(hr) => panic!("failed to create depth/stencil state: {}", HResult(hr)),
        _ => (),
    }
    let dss = Com::from_raw(dss);

    let default_rtbd = D3D11_RENDER_TARGET_BLEND_DESC {
        BlendEnable: FALSE,
        SrcBlend: D3D11_BLEND_ONE,
        DestBlend: D3D11_BLEND_ZERO,
        BlendOp: D3D11_BLEND_OP_ADD,
        SrcBlendAlpha: D3D11_BLEND_ONE,
        DestBlendAlpha: D3D11_BLEND_ZERO,
        BlendOpAlpha: D3D11_BLEND_OP_ADD,
        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL as u8,
    };
    let bd = D3D11_BLEND_DESC {
        AlphaToCoverageEnable: FALSE,
        IndependentBlendEnable: FALSE,
        RenderTarget: [
            D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: TRUE,
                SrcBlend: D3D11_BLEND_SRC_ALPHA,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_SRC_ALPHA,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL as u8,
            },
            default_rtbd, default_rtbd, default_rtbd,
            default_rtbd, default_rtbd, default_rtbd, default_rtbd,
        ]
    };

    let mut bs = ptr::null_mut();
    match device.CreateBlendState(&bd, &mut bs) {
        hr if FAILED(hr) => panic!("failed to create blend state: {}", HResult(hr)),
        _ => (),
    }
    let bs = Com::from_raw(bs);

    // constants

    let mut view = ptr::null_mut();
    let bd = D3D11_BUFFER_DESC {
        ByteWidth: mem::size_of::<View>() as UINT,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER,
        ..mem::zeroed()
    };
    match device.CreateBuffer(&bd, ptr::null_mut(), &mut view) {
        hr if FAILED(hr) => panic!("failed to create constant buffer: {}", HResult(hr)),
        _ => (),
    }
    let view = Com::from_raw(view);

    let mut material = ptr::null_mut();
    let bd = D3D11_BUFFER_DESC {
        ByteWidth: mem::size_of::<Material>() as UINT,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER,
        ..mem::zeroed()
    };
    match device.CreateBuffer(&bd, ptr::null_mut(), &mut material) {
        hr if FAILED(hr) => panic!("failed to create constant buffer: {}", HResult(hr)),
        _ => (),
    }
    let material = Com::from_raw(material);

    // texture

    let atlas = &assets.textures[0];
    let (width, height) = atlas.size;

    let mut texture = ptr::null_mut();
    let td = D3D11_TEXTURE2D_DESC {
        Width: width as u32,
        Height: height as u32,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_IMMUTABLE,
        BindFlags: D3D11_BIND_SHADER_RESOURCE,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let sd = D3D11_SUBRESOURCE_DATA {
        pSysMem: atlas.data.as_ptr() as *const _,
        SysMemPitch: width as u32 * 4,
        ..mem::zeroed()
    };
    match device.CreateTexture2D(&td, &sd, &mut texture) {
        hr if FAILED(hr) => panic!("failed to create texture: {}", HResult(hr)),
        _ => (),
    }
    let texture = Com::from_raw(texture);

    let mut srv = ptr::null_mut();
    let mut srvd = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
        ..mem::zeroed()
    };
    *srvd.u.Texture2D_mut() = D3D11_TEX2D_SRV {
        MostDetailedMip: 0,
        MipLevels: 1,
    };
    match device.CreateShaderResourceView(&**texture as *const _ as *mut _, &srvd, &mut srv) {
        hr if FAILED(hr) => panic!("failed to create srv: {}", HResult(hr)),
        _ => (),
    }
    let srv = Com::from_raw(srv);

    // vertex buffers

    let vertex_capacity = 100 * 6 * mem::size_of::<crate::batch::Vertex>() as UINT;
    let vertex_buffer = create_buffer(&device, vertex_capacity, D3D11_BIND_VERTEX_BUFFER);

    let index_capacity = 100 * 6 * mem::size_of::<u16>() as UINT;
    let index_buffer = create_buffer(&device, index_capacity, D3D11_BIND_INDEX_BUFFER);

    *graphics = Some(Draw {
        device, context, swap_chain, frame_wait, rtv, rtv_size,
        input_layout, sampler, vertex_shader, rs, pixel_shader, dss, bs,
        view, material, srv, vertex_buffer, index_buffer, vertex_capacity, index_capacity,
    });

    loop {
        let result = WaitForSingleObjectEx(frame_wait, INFINITE, TRUE);
        if result == WAIT_OBJECT_0 {
            break;
        } else if result == WAIT_TIMEOUT || result == WAIT_FAILED {
            panic!("failed to wait for vsync");
        }
    }
} }

fn create_rtv(
    device: &ID3D11Device, swap_chain: &IDXGISwapChain
) -> Com<ID3D11RenderTargetView> { unsafe {
    let mut back_buffer = ptr::null_mut();
    match swap_chain.GetBuffer(0, &ID3D11Texture2D::uuidof(), &mut back_buffer) {
        hr if FAILED(hr) => panic!("failed to get back buffer: {}", HResult(hr)),
        _ => (),
    }
    let back_buffer = Com::from_raw(back_buffer as *mut ID3D11Texture2D);

    let rtvd = D3D11_RENDER_TARGET_VIEW_DESC {
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
        ..mem::zeroed()
    };

    let mut rtv = ptr::null_mut();
    match device.CreateRenderTargetView(&**back_buffer as *const _ as *mut _, &rtvd, &mut rtv) {
        hr if FAILED(hr) => panic!("failed to create render target view: {}", HResult(hr)),
        _ => (),
    }
    Com::from_raw(rtv)
} }

fn create_buffer(device: &ID3D11Device, capacity: UINT, bind: UINT) -> Com<ID3D11Buffer> { unsafe {
    let mut buffer = ptr::null_mut();
    let bd = D3D11_BUFFER_DESC {
        ByteWidth: capacity,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: bind,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE,
        ..mem::zeroed()
    };
    match device.CreateBuffer(&bd, ptr::null(), &mut buffer) {
        hr if FAILED(hr) => panic!("failed to create buffer: {}", HResult(hr)),
        _ => (),
    }
    Com::from_raw(buffer)
} }

pub fn frame(cx: &mut crate::Context) { unsafe {
    let Context { world, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { platform, graphics, .. } = draw;
    let &mut crate::platform::Draw { hwnd, .. } = platform;
    let Draw {
        device, context, swap_chain, rtv, rtv_size,
        input_layout, sampler, vertex_shader, rs, pixel_shader, dss, bs,
        view,
        ..
    } = graphics.as_mut().unwrap();

    let mut rect = RECT { ..mem::zeroed() };
    GetClientRect(hwnd, &mut rect);
    if (rect.right, rect.bottom) != *rtv_size {
        context.OMSetRenderTargets(0, ptr::null(), ptr::null_mut());
        *rtv = None;

        match swap_chain.ResizeBuffers(
            0, 0, 0, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT
        ) {
            hr if FAILED(hr) => panic!("failed to resize swap chain: {}", HResult(hr)),
            _ => (),
        }
        *rtv = Some(create_rtv(device, swap_chain));
    }
    let rtv = rtv.as_mut().unwrap();

    let gray = 192.0 / 255.0;
    context.ClearRenderTargetView(rtv.as_ptr(), &[gray, gray, gray, 1.0]);

    let rtvs = [rtv.as_ptr()];
    context.OMSetRenderTargets(rtvs.len() as UINT, rtvs.as_ptr(), ptr::null_mut());

    // per-view

    let viewport = D3D11_VIEWPORT {
        TopLeftX: 0.0,
        TopLeftY: 0.0,
        Width: rect.right as f32,
        Height: rect.bottom as f32,
        MinDepth: 0.0,
        MaxDepth: 0.0,
    };
    context.RSSetViewports(1, &viewport);

    let dpi = GetDpiForWindow(hwnd);
    let scale = f32::ceil(dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32);
    let view_data = View {
        view_size: [viewport.Width / scale, viewport.Height / scale],
        port_size: [viewport.Width, viewport.Height],
    };
    context.UpdateSubresource(
        &***view as *const _ as *mut _, 0, ptr::null_mut(),
        &view_data as *const _ as *const _, 0, 0
    );

    let constants = [view.as_ptr()];
    context.VSSetConstantBuffers(0, constants.len() as UINT, constants.as_ptr());

    // per-material

    context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    context.IASetInputLayout(input_layout.as_ptr());
    let samplers = [sampler.as_ptr()];
    context.PSSetSamplers(0, samplers.len() as UINT, samplers.as_ptr());
    context.VSSetShader(vertex_shader.as_ptr(), ptr::null_mut(), 0);
    context.RSSetState(rs.as_ptr());
    context.PSSetShader(pixel_shader.as_ptr(), ptr::null_mut(), 0);
    context.OMSetDepthStencilState(dss.as_ptr(), 0);
    context.OMSetBlendState(bs.as_ptr(), &[1.0, 1.0, 1.0, 1.0], 0xffffffff);
} }

pub fn batch(cx: &mut crate::Context) { unsafe {
    let Context { world, assets, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { graphics, batch, .. } = draw;
    let Draw {
        device, context,
        vertex_buffer, index_buffer, vertex_capacity, index_capacity,
        material, srv,
        ..
    } = graphics.as_mut().unwrap();
    if batch.index.len() == 0 {
        return;
    }

    let atlas = &assets.textures[batch.texture as usize];
    let (width, height) = atlas.size;

    let material_data = Material { atlas_size: [width as f32, height as f32] };
    context.UpdateSubresource(
        &***material as *const _ as *mut _, 0, ptr::null_mut(),
        &material_data as *const _ as *const _, 0, 0
    );

    let constants = [material.as_ptr()];
    context.PSSetConstantBuffers(0, constants.len() as UINT, constants.as_ptr());

    let textures = [srv.as_ptr()];
    context.PSSetShaderResources(0, textures.len() as UINT, textures.as_ptr());

    update_buffer(
        device, context,
        vertex_buffer, D3D11_BIND_VERTEX_BUFFER, vertex_capacity, &batch.vertex[..]
    );
    update_buffer(
        device, context,
        index_buffer, D3D11_BIND_INDEX_BUFFER, index_capacity, &batch.index[..]
    );

    let buffers = [vertex_buffer.as_ptr()];
    let strides = [mem::size_of::<crate::batch::Vertex>() as UINT];
    let offsets = [0];
    context.IASetVertexBuffers(
        0, buffers.len() as UINT,
        buffers.as_ptr(), strides.as_ptr(), offsets.as_ptr()
    );
    context.IASetIndexBuffer(index_buffer.as_ptr(), DXGI_FORMAT_R16_UINT, 0);

    context.DrawIndexed(batch.index.len() as UINT, 0, 0);
} }

// Safety: capacity must represent buffer's actual size.
unsafe fn update_buffer<T: Copy>(
    device: &ID3D11Device, context: &ID3D11DeviceContext,
    buffer: &mut Com<ID3D11Buffer>, bind: UINT, capacity: &mut UINT, data: &[T]
) { unsafe {
    let needed = mem::size_of_val(data);
    if (*capacity as usize) < needed {
        assert!(needed <= UINT::MAX as usize);
        *capacity = cmp::max(2 * *capacity, needed as UINT);
        *buffer = create_buffer(device, *capacity, bind);
    }

    let mut ms = mem::MaybeUninit::uninit();
    match context.Map(
        &***buffer as *const _ as *mut _, 0, D3D11_MAP_WRITE_DISCARD, 0, ms.as_mut_ptr()
    ) {
        hr if FAILED(hr) => panic!("failed to map buffer: {}", HResult(hr)),
        _ => (),
    }
    let ptr = ms.assume_init().pData as *mut _;
    let len = data.len();
    let slice = slice::from_raw_parts_mut(ptr, len);
    slice.copy_from_slice(data);
    context.Unmap(&***buffer as *const _ as *mut _, 0);
} }

pub fn present(cx: &mut crate::Context) { unsafe {
    let Context { world, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { graphics, .. } = draw;
    let &mut Draw { ref swap_chain, frame_wait, .. } = graphics.as_mut().unwrap();

    // per-frame

    match swap_chain.Present(1, 0) {
        hr if FAILED(hr) => panic!("failed to swap: {}", HResult(hr)),
        _ => (),
    }
    loop {
        let result = WaitForSingleObjectEx(frame_wait, INFINITE, TRUE);
        if result == WAIT_OBJECT_0 {
            break;
        } else if result == WAIT_TIMEOUT || result == WAIT_FAILED {
            panic!("failed to wait for vsync");
        }
    }
} }
