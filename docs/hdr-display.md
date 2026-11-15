# HDR 디스플레이 출력 계획

Radiance HDR(`.hdr`)과 OpenEXR(`.exr`) 같은 고동적 범위 소스를 HDR 디스플레이의 확장 밝기 범위로 출력하는 기능의 계획 문서다. 기본 출력은 SDR이며, `ARAVIEW_HDR_OUTPUT=1` 개발 플래그로만 HDR 경로를 켤 수 있다(`SPEC.md` §18).

## 현재 상태(조사 근거)

앱은 gpui-kit 0.9.1을 쓰고, Windows 렌더러는 `gpui-pre-windows 0.3.12`(DirectX 11, 라이브러리 이름 `gpui_windows`)다. gpui-kit 0.9.1은 `gpui-pre-platform`을 거쳐 이 크레이트를 쓰며, 레지스트리에 함께 있는 `gpui-fast-windows`는 쓰지 않는다(`cargo tree -i gpui-pre-windows`). 확인한 코드 상태:

| 항목 | 상태 | 근거 |
| --- | --- | --- |
| 스왑체인 형식 | `B8G8R8A8_UNORM` 고정(8비트 sRGB) | `vendor/gpui-pre-windows/src/directx_renderer.rs`의 `RENDER_TARGET_FORMAT`, `create_swap_chain_for_composition`/`create_swap_chain` |
| 색 공간 제어 | `SetColorSpace1` 호출 없음, `DXGI_OUTPUT_DESC1` 조회 없음 | 같은 크레이트 전역 검색 |
| 스왑체인 생성 | DirectComposition + FLIP_SEQUENTIAL | `create_swap_chain_for_composition` |
| 최종 출력 인코딩 | 셰이더가 선형 합성 후 마지막에 sRGB 인코딩 | `vendor/gpui-pre-windows/src/shaders.hlsl`의 `linear_to_srgb` |
| 이미지 텍스처 | 8비트 BGRA만. gpui-pre 코어의 `RenderImage`/`Frame`도 BGRA 8비트 가정 | `gpui-pre/src/assets.rs`, 앱은 `app/src/picture.rs`의 `render_image`에서 `image::Frame`을 넘김 |
| 플랫폼 선택 | Windows는 `gpui-pre-windows` 고정. wgpu 백엔드는 Linux/wasm에만 연결 | `gpui-pre-platform`의 target별 dependency와 `gpui_platform.rs` |
| macOS EDR | `wantsExtendedDynamicRangeContent` 같은 설정 없음 | `gpui-pre-macos/src` 전역 검색 |
| wgpu | 트리에 있는 `wgpu 29.0.4`에는 HDR 표면 색 공간 API가 없음 | `SurfaceColorSpace`/`DisplayHdrInfo` 검색 불발(병합 전 상태) |

결론: 렌더러가 스왑체인 색 공간을 통제하지 않으므로 HDR 모니터에서도 SDR 밝기로만 그려진다. Radiance·EXR 디코드는 선형 부동소수라 데이터 자체는 준비돼 있지만, 표시 파이프라인(8비트 clamp, `raster_sidecar.rs`)에서 SDR로 내려간다.

## 외부 참고

- MSDN "High dynamic range display development": scRGB는 `R16G16B16A16_FLOAT` + `DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709`, HDR10은 `R10G10B10A2` + `DXGI_COLOR_SPACE_RGB_FULL_G2084_MINUS_P2020`. 색 공간은 반드시 명시적으로 지정한다.
- DXVK #3910: FP16 스왑체인에서 `SetColorSpace1`을 명시하지 않으면 컴포지터가 다른 밝기·감마로 보이고, 프레임에 음수 값이 섞이면 Windows 11 컴포지터가 상태를 망가뜨린다(음수 값 금지).
- wgpu #9658: 표면 색 공간 선택 API 설계(SRGB, ExtendedSrgbLinear=scRGB, Hdr10, Hlg). DX12 백엔드는 `IDXGIOutput6::GetDesc1`로 지원 판정 후 create/resize 양쪽에서 `SetColorSpace1`을 적용하고, 표시 정보 조회(`DisplayHdrInfo`)는 후속 #9739로 나눴다.
- wgpu 예제 `03_hdr_surface`: 50/100/203/400/1000/10000 nit 스텝 패턴으로 HDR 출력 검증. SDR 출력에서는 100 nit 위가 모두 잘려 눈으로 구분된다.
- Zed gpui 텍스트 감마 논의: 선형 공간 합성은 Windows 텍스트 렌더링(감마 1.8, 명암 보정)과 어긋난다. UI는 SDR 매핑으로 유지하는 근거.

## 설계 방향

표면 색 공간은 **scRGB 선형**(`R16G16B16A16_FLOAT` + `G10`)을 기본으로 한다.

- gpui 셰이더가 이미 선형 공간에서 합성하므로 최종 단계만 "SDR 흰색 배율 곱"으로 바꾸면 UI를 그대로 재사용한다. HDR10/PQ는 UI 색까지 PQ로 인코딩해야 해서 변경이 크다.
- Windows DWM은 1703부터 scRGB 합성을 지원하고, 스왑체인에서 Rec.2020/PQ 변환은 DWM이 한다.
- 이미지 콘텐츠(Radiance·EXR의 선형 float)를 재인코딩 없이 그대로 담는다.

단 scRGB는 플로트 스왑체인을 요구하므로 텍스처도 float 경로가 필요하다. HDR10(`R10G10B10A2` + PQ)은 8비트 업로드로도 가능하지만 UI 인코딩 비용이 커서 후순위로 둔다.

SDR 흰색 매핑: scRGB에서 SDR 기준 흰색은 80 nit 계열 기준값을 따른다. 디스플레이별 "SDR 콘텐츠 밝기" 설정은 `DXGI_OUTPUT_DESC1.SdrWhiteNits`(또는 Windows.Graphics.Display.AdvancedColorInfo)로 조회해 UI에 곱하고, 이미지는 원본 선형 광도를 그대로 쓴다.

## 구현 현황

### 실험 이력 (2026-10-19)

- **M1: hwnd + FP16 scRGB**: `CreateSwapChainForHwnd` + `R16G16B16A16_FLOAT` + `SetColorSpace1(G10)`에서 HDR 출력 성공. CPU 프로브로 0.5/1.0/50.0 값이 버퍼에 그려짐을 확인했고, DisplayHDR 400 패널에서 3밴드가 뚜렷하게 구분됐다.
- **M2: composition 경로 실패**: DirectComposition 스왑체인에 같은 FP16 scRGB를 올리면 DWM이 버퍼를 표시하지 않는다. `WS_EX_NOREDIRECTIONBITMAP`, overlapped 스타일, `Present(0, 0)`을 모두 시도해도 같고, FLIP_SEQUENTIAL composition에서는 `GetCurrentBackBufferIndex`가 항상 0을 반환한다. composition + BGRA8 SDR은 정상 동작한다.
- **결론과 포크**: composition 경로를 유지하지 않고 hwnd 경로를 HDR 기본으로 쓴다. `vendor/gpui-pre-windows`에 `gpui_windows::set_hdr_output(bool)`을 추가해 HDR 모드에서 `CreateSwapChainForHwnd`를 쓰고 `WS_EX_NOREDIRECTIONBITMAP`을 뺀다. `ARAVIEW_HDR_OUTPUT=1`이면 GPUI 플랫폼 생성 전에 `platform::prepare_hdr_output`이 이 함수를 호출한다.
- **FP16 출력**: `vendor/gpui-pre-windows/src/hdr_output.rs`. HDR 스왑체인은 `R16G16B16A16_FLOAT` + `DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709`. 장면은 기존 BGRA8 오프스크린에 그린 뒤 Present 직전 전체 화면 패스로 sRGB EOTF 선형화와 SDR 흰색 배율을 적용한다. 배율은 `DisplayConfigGetDeviceInfo(DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL)`의 `SDRWhiteLevel / 1000`(1.0 = 80 nit)이고 120프레임마다 다시 읽는다. 변환 셰이더는 빌드 스크립트를 건드리지 않으려고 런타임 `D3DCompile`로 컴파일한다.
- **M3: 이미지 표시**: HDR 모드 오프스크린을 `R16G16B16A16_FLOAT`로 올리고, SDR 흰색까지는 그대로, 그 위는 `white + (peak - white) * (1 - exp(-over / (peak - white)))`로 패널 피크에 접근한다. 피크는 `IDXGIOutput6::GetDesc1().MaxLuminance`(출력 색 공간이 G2084일 때만)를 쓰고, 모르면 SDR 흰색에서 자른다.
- **M3 텍스처·디코드**: `gpui_windows::register_hdr_image(image_id, w, h, rgba_f16)`가 이미지별 FP16 픽셀을 등록하면 아틀라스가 RGBA16F 텍스처에 올린다. `raster_sidecar::decode_linear`가 OpenEXR·Radiance HDR을 `Rgba32F`로 디코드하고, `picture.rs`가 방향·축소·타일을 SDR과 같게 만든 뒤 선형 값을 sRGB 곡선으로 인코딩해 f16으로 등록한다. 애니메이션과 크기가 다른 렌더 사본은 SDR로 둔다.
- **런타임 확인**: SDR 흰색 280 nit, 피크 400 nit. sample.hdr(32x16)·sample.exr(400x300)이 RGBA16F로 등록됨. HDR 모드 selftest 21항목 통과. `samples/sample-hdr-bands.hdr`(700x300, 확인 당시 1600x900)의 0.5/1.0/1.2/1.4/2/4/8배 띠에서 SDR 흰색 위가 더 밝게 구분됨을 사용자가 눈으로 확인했다. SDR 캡처는 80 nit 초과를 잘라 판정에 쓸 수 없다.

### 남은 계획

- **M4 — 앱 통합**: HDR 소스는 float 타일을 쓰고 썸네일·히스토그램은 8비트를 유지한다. 디스플레이 피크 기준 톤매핑, `hdrOutput` 설정(§13), 필요 시 §14 단축키·팔레트, SPEC §2/§13/§18과 `samples/` 검증 항목 갱신.
- composition + scRGB 원인 조사, 노출 조절, 설정 토글, 큰 EXR의 메모리(f32 디코드 + f16 사본) 상한은 후속 과제로 남긴다.
- 종료 조건: HDR 모니터에서 Radiance·EXR 하이라이트가 SDR과 구분되고, SDR 모니터·HDR 끔 상태에서는 기존 표시와 동일하다.

## 포크·업스트림 전략

- 패치는 `vendor/`에 두고 `Cargo.toml`의 `[patch.crates-io]`로 고정해 gpui-kit 버전을 올릴 때 재베이스한다.
- gpui-kit 버전 갱신 시 패치 지면(스왑체인 생성, 셰이더 출력, 텍스처 업로드)이 좁게 유지되도록 최소한의 인터페이스로 격리한다.
- 안정화 후 longbridge/gpui-kit에 (1) Windows HDR 표면 색 공간, (2) float 이미지 텍스처 두 건으로 나눠 PR을 제안한다. 업스트림 반영되면 패치를 제거한다.

## 리스크

| 리스크 | 대응 |
| --- | --- |
| HDR 모니터 부재로 검증 불가 | M1에서 중단, 계획 보류(SPEC에는 미지원으로 유지) |
| gpui-kit 업데이트마다 패치 충돌 | 지면 최소화 + 업스트림 PR 조기 제안 |
| UI 명도 변화(선형 합성 + 스케일 누락) | M2 종료 조건에서 기존 명도 비교(캡처 mean 값) |
| 컴포지터 상태 파괴(음수/NaN) | 디코딩·톤매핑에서 음수 클램프 어설션 |
| 16F 스왑체인 대역폭 증가 | HDR 모니터에서만 전환, 캡처·성능 측정 |
| 텍스트 렌더링(Windows 감마 1.8) 변화 | UI를 SDR 흰색 배율 매핑으로 유지해 현재와 동일한 감마 동작 보존 |
| composition 경로 블로커 | hwnd swapchain으로 폴백(M2 결론) |

## 검증 방법

- `pwsh scripts/araview-drive.ps1`로 앱 제어(selftest는 SDR 경로 검증용).
- HDR 출력 자체는 SDR 캡처에 나타나지 않으므로: (1) nit 스텝 패턴을 HDR 모니터에서 사람이 확인, (2) `DXGI_OUTPUT_DESC1` 조회 값 기록, (3) SDR 캡처에서는 클리핑 값만 확인.
- Windows 설정: 시스템 > 디스플레이 > HDR 켜기/끄기를 바꿔가며 폴백 확인.
