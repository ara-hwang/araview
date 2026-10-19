// AraView fork: HDR 출력(docs/hdr-display.md M2).
//
// GPUI는 sRGB 인코딩 값을 감마 공간에서 블렌딩한다. HDR 모드에서도 UI 모양을 그대로 두려고
// 장면은 지금처럼 BGRA8 오프스크린 타깃에 그리고, Present 직전에 한 번의 전체 화면 패스로
// FP16 스왑체인(scRGB, 선형, 1.0 = 80 nit)에 옮긴다. 이때 SDR 흰색 레벨을 곱해 Windows의
// "SDR 콘텐츠 밝기" 설정과 같은 밝기로 보이게 한다.

use std::{ffi::c_void, mem::size_of, slice};

use anyhow::{Context, Result};
use windows::{
    Win32::{
        Devices::Display::{
            DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
            DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
            DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SDR_WHITE_LEVEL,
            DISPLAYCONFIG_SOURCE_DEVICE_NAME, DisplayConfigGetDeviceInfo,
            GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
        },
        Foundation::{ERROR_SUCCESS, HWND},
        Graphics::{
            Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, Fxc::D3DCompile, ID3DBlob},
            Direct3D11::*,
            Dxgi::{Common::*, IDXGISwapChain1, IDXGISwapChain3},
            Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFOEXW, MonitorFromWindow},
        },
    },
    core::{Interface, PCSTR, s},
};

/// HDR 모드의 스왑체인 형식.
pub(crate) const HDR_SWAP_CHAIN_FORMAT: DXGI_FORMAT = DXGI_FORMAT_R16G16B16A16_FLOAT;

/// SDR 흰색 레벨을 다시 읽는 간격(프레임). 창이 다른 모니터로 옮겨 가도 따라가게 한다.
const WHITE_LEVEL_REFRESH_FRAMES: u32 = 120;

const SHADER: &str = r#"
cbuffer Params : register(b0) {
    float sdr_white_scale;
    float3 _pad;
};
Texture2D<float4> scene : register(t0);

float4 hdr_vertex(uint id : SV_VertexID) : SV_Position {
    float2 uv = float2((id << 1) & 2, id & 2);
    return float4(uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
}

// Windows가 SDR 앱을 HDR 화면에 합성할 때처럼 구간별 sRGB EOTF로 선형화한다.
float3 srgb_eotf(float3 c) {
    c = saturate(c);
    return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4);
}

float4 hdr_fragment(float4 position : SV_Position) : SV_Target {
    float4 c = scene.Load(int3(position.xy, 0));
    return float4(srgb_eotf(c.rgb) * sdr_white_scale, 1.0);
}
"#;

#[repr(C)]
struct Params {
    sdr_white_scale: f32,
    _pad: [f32; 3],
}

/// 스왑체인에 scRGB 색 공간을 지정한다.
pub(crate) fn set_scrgb_color_space(swap_chain: &IDXGISwapChain1) -> Result<()> {
    let swap_chain: IDXGISwapChain3 = swap_chain.cast()?;
    unsafe { swap_chain.SetColorSpace1(DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709) }
        .context("SetColorSpace1(scRGB)")
}

/// 장면을 그릴 오프스크린 타깃. 셰이더가 읽을 수 있게 SRV도 만든다.
pub(crate) fn create_scene_target(
    device: &ID3D11Device,
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
) -> Result<(ID3D11Texture2D, Option<ID3D11RenderTargetView>)> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture))? };
    let texture = texture.context("creating HDR scene target")?;
    let mut view = None;
    unsafe { device.CreateRenderTargetView(&texture, None, Some(&mut view))? };
    Ok((texture, view))
}

/// 크기마다 다시 만드는 HDR 리소스: 스왑체인 백버퍼 RTV와 장면 SRV.
pub(crate) struct HdrTargets {
    backbuffer_view: Option<ID3D11RenderTargetView>,
    scene_view: Option<ID3D11ShaderResourceView>,
}

impl HdrTargets {
    pub(crate) fn new(
        device: &ID3D11Device,
        swap_chain: &IDXGISwapChain1,
        scene: &ID3D11Texture2D,
    ) -> Result<Self> {
        let backbuffer: ID3D11Texture2D = unsafe { swap_chain.GetBuffer(0) }?;
        let mut backbuffer_view = None;
        unsafe { device.CreateRenderTargetView(&backbuffer, None, Some(&mut backbuffer_view))? };
        let mut scene_view = None;
        unsafe { device.CreateShaderResourceView(scene, None, Some(&mut scene_view))? };
        Ok(Self {
            backbuffer_view,
            scene_view,
        })
    }
}

/// 장치마다 한 번 만드는 변환 패스.
pub(crate) struct HdrPass {
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    params: Option<ID3D11Buffer>,
    hwnd: HWND,
    sdr_white_scale: f32,
    frames_since_refresh: u32,
}

impl HdrPass {
    pub(crate) fn new(device: &ID3D11Device, hwnd: HWND) -> Result<Self> {
        let vs_blob = compile(s!("hdr_vertex"), s!("vs_4_1"))?;
        let ps_blob = compile(s!("hdr_fragment"), s!("ps_4_1"))?;
        let mut vertex_shader = None;
        let mut pixel_shader = None;
        let mut params = None;
        unsafe {
            device.CreateVertexShader(blob_bytes(&vs_blob), None, Some(&mut vertex_shader))?;
            device.CreatePixelShader(blob_bytes(&ps_blob), None, Some(&mut pixel_shader))?;
            let desc = D3D11_BUFFER_DESC {
                ByteWidth: size_of::<Params>() as u32,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                MiscFlags: 0,
                StructureByteStride: 0,
            };
            device.CreateBuffer(&desc, None, Some(&mut params))?;
        }
        let sdr_white_scale = sdr_white_scale(hwnd);
        log::info!("HDR output enabled, SDR white scale {sdr_white_scale}");
        Ok(Self {
            vertex_shader: vertex_shader.context("HDR vertex shader")?,
            pixel_shader: pixel_shader.context("HDR pixel shader")?,
            params,
            hwnd,
            sdr_white_scale,
            frames_since_refresh: 0,
        })
    }

    /// 장면을 백버퍼로 옮긴다. 다음 프레임의 `pre_draw`가 렌더 타깃과 상수 버퍼를 다시 묶는다.
    pub(crate) fn run(
        &mut self,
        context: &ID3D11DeviceContext,
        targets: &HdrTargets,
        viewport: &D3D11_VIEWPORT,
    ) -> Result<()> {
        self.frames_since_refresh += 1;
        if self.frames_since_refresh >= WHITE_LEVEL_REFRESH_FRAMES {
            self.frames_since_refresh = 0;
            self.sdr_white_scale = sdr_white_scale(self.hwnd);
        }
        let params = self.params.as_ref().context("HDR params buffer")?;
        unsafe {
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            context.Map(params, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))?;
            mapped.pData.cast::<Params>().write(Params {
                sdr_white_scale: self.sdr_white_scale,
                _pad: [0.0; 3],
            });
            context.Unmap(params, 0);

            context.OMSetRenderTargets(Some(slice::from_ref(&targets.backbuffer_view)), None);
            context.OMSetBlendState(None, None, 0xFFFFFFFF);
            context.RSSetViewports(Some(slice::from_ref(viewport)));
            context.IASetInputLayout(None);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetShader(&self.vertex_shader, None);
            context.PSSetShader(&self.pixel_shader, None);
            context.PSSetConstantBuffers(0, Some(slice::from_ref(&self.params)));
            context.PSSetShaderResources(0, Some(slice::from_ref(&targets.scene_view)));
            context.Draw(3, 0);
            // 다음 프레임에 장면 텍스처를 렌더 타깃으로 다시 묶을 수 있게 푼다.
            context.PSSetShaderResources(0, Some(&[None]));
        }
        Ok(())
    }
}

fn compile(entry: PCSTR, target: PCSTR) -> Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    let result = unsafe {
        D3DCompile(
            SHADER.as_ptr() as *const c_void,
            SHADER.len(),
            s!("hdr_output.hlsl"),
            None,
            None,
            entry,
            target,
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    if let Err(error) = result {
        let message = errors
            .map(|blob| String::from_utf8_lossy(blob_bytes(&blob)).into_owned())
            .unwrap_or_default();
        anyhow::bail!("HDR shader compile failed: {error} {message}");
    }
    code.context("HDR shader blob")
}

fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    // SAFETY: 블롭이 살아 있는 동안 포인터와 크기가 가리키는 메모리는 유효하다.
    unsafe { slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize()) }
}

/// 창이 있는 모니터의 SDR 흰색 레벨을 scRGB 배율로 돌려준다(1.0 = 80 nit).
/// 조회에 실패하면 배율 1.0(80 nit)으로 둔다.
fn sdr_white_scale(hwnd: HWND) -> f32 {
    query_sdr_white_level(hwnd)
        .map(|level| level as f32 / 1000.0)
        .filter(|scale| scale.is_finite() && *scale > 0.0)
        .unwrap_or(1.0)
}

fn query_sdr_white_level(hwnd: HWND) -> Option<u32> {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(monitor, &mut info.monitorInfo).as_bool() {
            return None;
        }

        let mut path_count = 0;
        let mut mode_count = 0;
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
            != ERROR_SUCCESS
        {
            return None;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        ) != ERROR_SUCCESS
        {
            return None;
        }

        for path in &paths[..path_count as usize] {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: path.sourceInfo.adapterId,
                    id: path.sourceInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut source.header) != ERROR_SUCCESS.0 as i32 {
                continue;
            }
            if source.viewGdiDeviceName != info.szDevice {
                continue;
            }
            let mut white = DISPLAYCONFIG_SDR_WHITE_LEVEL {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
                    size: size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>() as u32,
                    adapterId: path.targetInfo.adapterId,
                    id: path.targetInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut white.header) != ERROR_SUCCESS.0 as i32 {
                return None;
            }
            return Some(white.SDRWhiteLevel);
        }
        None
    }
}
