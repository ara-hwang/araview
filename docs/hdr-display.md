# HDR 디스플레이 출력 계획

Radiance HDR(`.hdr`)과 OpenEXR(`.exr`) 같은 고동적 범위 소스를 HDR 디스플레이의 확장 밝기 범위로 출력하는 기능의 계획 문서다. 작성 시점(2026-10-19)에는 지원하지 않으며, 이 문서는 도입을 위한 조사 결과와 단계별 계획을 담는다.

## 현재 상태(조사 근거)

앱은 gpui-kit 0.7.3을 쓰고, Windows 렌더러는 `gpui-pre-windows 0.3.9`(DirectX 11, 라이브러리 이름 `gpui_windows`)다. gpui-kit 0.7.3은 `gpui-pre-platform`을 거쳐 이 크레이트를 쓰며, 레지스트리에 함께 있는 `gpui-fast-windows`는 쓰지 않는다(`cargo tree -i gpui-pre-windows`). 확인한 코드 상태:

| 항목 | 상태 | 근거 |
| --- | --- | --- |
| 스왑체인 형식 | `B8G8R8A8_UNORM` 고정(8비트 sRGB) | `vendor/gpui-pre-windows/src/directx_renderer.rs`의 `RENDER_TARGET_FORMAT`, `create_swap_chain_for_composition`/`create_swap_chain` |
| 색 공간 제어 | `SetColorSpace1` 호출 없음, `DXGI_OUTPUT_DESC1` 조회 없음 | 같은 크레이트 전역 검색 |
| 스왑체인 생성 | DirectComposition + FLIP_SEQUENTIAL | `create_swap_chain_for_composition` |
| 최종 출력 인코딩 | 셰이더가 선형 합성 후 마지막에 sRGB 인코딩 | `vendor/gpui-pre-windows/src/shaders.hlsl`의 `linear_to_srgb` |
| 이미지 텍스처 | 8비트 BGRA만. gpui-pre 코어의 `RenderImage`/`Frame`도 BGRA 8비트 가정 | `gpui-pre-0.3.9/src/assets.rs`, 앱은 `src-gpui/src/picture.rs`의 `render_image`에서 `image::Frame`을 넘김 |
| 플랫폼 선택 | Windows는 `gpui-pre-windows` 고정. wgpu 백엔드는 Linux/wasm에만 연결 | `gpui-pre-platform-0.3.9`의 target별 dependency와 `gpui_platform.rs` |
| macOS EDR | `wantsExtendedDynamicRangeContent` 같은 설정 없음 | `gpui-pre-macos-0.3.9/src` 전역 검색 |
| wgpu | 트리에 있는 `wgpu 29.0.4`에는 HDR 표면 색 공간 API가 없음 | `SurfaceColorSpace`/`DisplayHdrInfo` 검색 불발(병합 전 상태) |

결론: 렌더러가 스왑체인 색 공간을 통제하지 않으므로 HDR 모니터에서도 SDR 밝기로만 그려진다. Radiance·EXR 디코드는 선형 부동소수라 데이터 자체는 준비돼 있지만, 표시 파이프라인(8비트 clamp, `raster_sidecar.rs`)에서 SDR로 내려간다.

## 외부 참고

- MSDN "High dynamic range display development": scRGB는 `R16G16B16A16_FLOAT` + `DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709`, HDR10은 `R10G10B10A2` + `DXGI_COLOR_SPACE_RGB_FULL_G2084_MINUS_P2020`. 색 공간은 반드시 명시적으로 지정한다.
- DXVK #3910: FP16 스왑체인에서 `SetColorSpace1`을 명시하지 않으면 컴포지터가 다른 밝기·감마로 보이고, 프레임에 음수 값이 섞이면 Windows 11 컴포지터가 상태를 망가뜨린다(음수 값 금지).
- wgpu #9658: 표면 색 공간 선택 API 설계(SRGB, ExtendedSrgbLinear=scRGB, Hdr10, Hlg). DX12 백엔드는 `IDXGIOutput6::GetDesc1`로 지원 판정 후 create/resize 양쪽에서 `SetColorSpace1`을 적용하고, 표시 정보 조회(`DisplayHdrInfo`)는 후속 #9739로 나눴다. 설계 참고로 쓴다.
- wgpu 예제 `03_hdr_surface`: 50/100/203/400/1000/10000 nit 스텝 패턴으로 HDR 출력 검증하는 방법. SDR 출력에서는 100 nit 위가 모두 잘리므로 눈으로 구분된다.
- Zed gpui 텍스트 감마 논의: 선형 공간 합성은 Windows 텍스트 렌더링(감마 1.8, 명암 보정)과 어긋난다. UI는 SDR 매핑으로 유지하는 근거.

## 설계 방향

표면 색 공간은 **scRGB 선형**(`R16G16B16A16_FLOAT` + `G10`)을 기본으로 한다. 근거:

- gpui 셰이더가 이미 선형 공간에서 합성하므로 최종 단계만 "SDR 흰색 배율 곱"으로 바꾸면 UI를 그대로 재사용한다. HDR10/PQ는 UI 색까지 PQ로 인코딩해야 해서 변경이 크다.
- Windows DWM은 1703부터 scRGB 합성을 지원하고, 스왑체인에서 Rec.2020/PQ 변환은 DWM이 한다.
- 이미지 콘텐츠(Radiance·EXR의 선형 float)를 재인코딩 없이 그대로 담는다.

단 scRGB는 플로트 스왑체인을 요구하므로 텍스처도 float 경로가 필요하다. HDR10(`R10G10B10A2` + PQ)은 8비트 업로드로도 가능하지만 UI 인코딩 비용이 커서 후순위로 둔다.

SDR 흰색 매핑: scRGB에서 SDR 기준 흰색은 80 nit 계열 기준값을 따른다. 디스플레이별 "SDR 콘텐츠 밝기" 설정은 `DXGI_OUTPUT_DESC1.SdrWhiteNits`(또는 Windows.Graphics.Display.AdvancedColorInfo)로 조회해 UI에 곱하고, 이미지는 원본 선형 광도를 그대로 쓴다.

## 단계별 계획

### M1 — 독립 실험(위험 제거) — **결과 (2026-10-19)**

M1을 실행했다. 결과:

- **hwnd path + FP16 + SetColorSpace1(scRGB G10)**: HDR 출력 성공. 사용자 패널(DisplayHDR 400, 400 nit peak / 160 nit full-frame, Rec.709-ish primaries)에서 3밴드(0.5× / 1.0× SDR 흰색 / 50×)가 뚜렷하게 구분됨. CPU 프로브로도 스왑체인 버퍼에 정확한 scRGB 값(0.5 / 1.0 / 50.0)이 그려짐을 확인.
- **composition path (CreateSwapChainForComposition + DComp)**: 실패. CPU 프로브는 정확한 값이 버퍼 0에 그려짐을 보이지만, DWM은 그 버퍼를 보여주지 않는다(사용자에게는 균일한 검정/흰색으로 보임). `GetCurrentBackBufferIndex`는 FLIP_SEQUENTIAL composition 스왑체인에서 항상 0을 반환해 버퍼 회전을 추적할 수 없다. gpui 스타일의 2단계 visual tree(target → root → swapchain visual)도 동일한 증상.
- **결론**: scRGB 출력 자체는 동작한다. gpui가 쓰는 composition 경로가 HDR 출력의 실제 블로커다.

M1 종료 조건 충족: HDR 모니터에서 scRGB 출력 성공 (hwnd path). composition 경로는 M2로 이월.

## M2 — gpui-pre-windows 패치(스왑체인·디스플레이 정보) — **재작성**

composition 경로 블로커를 반영해 접근을 재정의한다.

1. **1차 시도: composition 버퍼 회전 원인 규명(1-2일)**
   - `IDXGISwapChain3::GetCurrentBackBufferIndex`가 항상 0을 반환하는 이유를 조사한다. composition 스왑체인에서 `GetBuffer(1)`이 `E_INVALIDARG`를 반환하는지, `GetBuffer(0)`가 매 Present마다 같은 객체인지 확인한다.
   - gpui의 SDR composition이 동작하는 이유를 비교한다(gpui는 단일 RTV on buffer 0 + `Present(0, 0)`). 동일 패턴이 SDR에서 동작하는지 composition 테스트 앱으로 확인한다.
   - gpui의 `Present(0, 0)`(vsync 0) vs 우리 테스트의 `Present(1, 0)`(vsync 1) 차이도 검증한다.
2. **2차 시도: hwnd swapchain으로 base surface 전환(폴백)**
   - composition 회전이 해결되지 않으면, base swapchain을 `CreateSwapChainForHwnd`로 전환한다. DirectComposition layering(Mica/Acrylic, 오버레이)은 별도 concern으로 분리.
   - 이 경우 투명 창/오버레이 기능 상실을 감수하고, HDR 출력을 우선한다.
3. **공통 작업**
   - 디스플레이 정보: `DXGI_OUTPUT_DESC1`(색 공간, 피크 광도, full-frame 광도) + Windows.Graphics.Display.AdvancedColorInfo로 HDR 지원·피크·SDR 흰색 광도 조회.
   - 셰이더: 출력 인코딩 분기(sRGB / scRGB 스케일). UI는 SDR 흰색 배율 매핑으로 기존 명도 유지.
   - SDR 폴백: HDR 미지원·미설정이면 현재 동작(B8G8R8A8) 그대로.

### M2 결과 (2026-10-19)

M2 1차 시도(composition 버퍼 회전 원인 규명)를 실행했다. 결과:

- **hwnd path + FP16 + SetColorSpace1(scRGB G10)**: HDR 출력 성공 (M1에서 확인). 3밴드 구분됨.
- **composition + FP16 + SetColorSpace1(scRGB G10)**: 실패. DWM이 렌더링한 버퍼를 보여주지 않음. WS_EX_NOREDIRECTIONBITMAP 추가, overlapped window 스타일, Present(0,0)(vsync 0) 모두 동일한 증상. GetCurrentBackBufferIndex는 FLIP_SEQUENTIAL composition에서 항상 0 반환.
- **composition + BGRA8 (SDR)**: AraView(gpui)에서는 정상 동작. 테스트 앱에서는 창이 보이지 않음(가시성 문제로 추정).
- **gpui의 SDR composition 동작 원리**: gpui는 단일 RTV on buffer 0 + Present(0, 0) + WS_EX_NOREDIRECTIONBITMAP을 사용. SDR에서 동작. 동일 패턴이 FP16에서는 동작하지 않음.

**결론**: composition 경로에서 scRGB FP16 콘텐츠가 DWM에 의해 표시되지 않는다. hwnd 경로에서는 정상 동작. 이는 composition 스왑체인의 DWM 처리 방식과 scRGB 색 공간의 조합에서 발생하는 문제로 추정.

### M2 재작성 (2차 시도)

composition 경로 블로커가 해결되지 않으므로 접근을 변경한다.

1. **hwnd swapchain으로 base surface 전환 (주 접근)**
   - base swapchain을 CreateSwapChainForHwnd로 전환한다. 이 경로에서 scRGB FP16 + SetColorSpace1이 정상 동작함을 M1에서 확인.
   - GPUI_DISABLE_DIRECT_COMPOSITION 환경 변수로 이미 hwnd 경로 전환 가능. HDR 모드에서 자동으로 hwnd 경로를 사용하도록 변경.
   - DirectComposition layering(Mica/Acrylic, 오버레이)은 별도 concern으로 분리. 투명 창/오버레이 기능 상실을 감수.
2. **composition 블로커 연구 (병행)**
   - composition + scRGB가 실패하는 이유를 조사한다. 가능성:
     - DWM이 composition 스왑체인에서 scRGB(G10) 콘텐츠를 올바르게 변환하지 못함 (PQ/G2084 형식 필요?)
     - composition 스왑체인의 버퍼 회전이 SDR과 HDR에서 다르게 동작
   - 이 연구가 성공하면 composition 경로를 유지할 수 있다.
3. **공통 작업**
   - 디스플레이 정보: DXGI_OUTPUT_DESC1 + AdvancedColorInfo로 HDR 지원·피크·SDR 흰색 광도 조회.
   - 셰이더: 출력 인코딩 분기(sRGB / scRGB 스케일).
   - SDR 폴백: HDR 미지원·미설정이면 현재 동작(B8G8R8A8) 그대로.

### M3 — float 텍스처 경로(이미지 콘텐츠)

8비트 BGRA `RenderImage`만 있는 코어에, 이미지 캔버스 타일만 16F 선형 광도를 올리는 경로를 만든다.

- 직접 경로: gpui-pre 코어(`assets.rs`의 Frame/RenderImage)에 16F 프레임 추가 + 두 플랫폼 렌더러 텍스처 업로드 확장. 변경 범위가 커서 업스트림 합의 대상.
- 우회 경로: 16F 반쪽 값을 RGBA8 2장(상·하위 평면)으로 분할해 업로드하고 셰이더에서 재조립. 코어 타입을 건드리지 않아 패치가 렌더러·셰이더에만 머문다. 우선 우회 경로로 검증하고, 업스트림 논의 후 직접 경로로 이동한다.

종료 조건: Radiance·EXR 원본의 선형 광도가 텍스처까지 보존됨(8비트 clamp 없음)을 단위 테스트로 확인.

### M4 — 앱 통합

1. 디코드: Radiance·EXR을 8비트 clamp가 아니라 선형 float(반쪽)로 유지하는 변형 경로를 만들고, HDR 소스는 JPEG sidecar(8비트) 대신 float 타일을 쓴다. 썸네일·히스토그램은 8비트 경로 유지.
2. 톤매핑: 디스플레이 피크 광도 기준으로 소스 최대값을 매핑한다. SDR 모니터에서는 현재 clamp+sRGB를 유지한다.
3. 설정: `hdrOutput`(자동/끄기) 추가(§13), 단축키·팔레트 연결 필요 시 §14.
4. 문서: SPEC §2(가능하면 HDR 소스 표기), §13, §18 갱신, `samples/` 검증 항목 추가.

종료 조건: HDR 모니터에서 Radiance·EXR 하이라이트가 SDR 표시와 구분되고, SDR 모니터·HDR 끔 상태에서 기존 표시와 동일하다.

## 포크·업스트림 전략

- 패치는 `vendor/`에 두고 `Cargo.toml`의 `[patch.crates-io]`로 고정해 gpui-kit 버전을 올릴 때 재베이스한다.
- gpui-kit 버전 갱신 시: 패치 지면(스왑체인 생성, 셰이더 출력, 텍스처 업로드)이 좁게 유지되도록 최소한의 인터페이스로 격리한다.
- 안정화 후 longbridge/gpui-kit에 두 건으로 나눠 PR을 제안한다: (1) Windows HDR 표면 색 공간, (2) float 이미지 텍스처. 업스트림 반영되면 패치를 제거한다.

## 리스크

| 리스크 | 대응 |
| --- | --- |
| HDR 모니터 부재로 검증 불가 | M1에서 중단, 계획 보류(SPEC에는 미지원으로 유지) |
| gpui-kit 업데이트마다 패치 충돌 | 지면 최소화 + 업스트림 PR 조기 제안 |
| UI 명도 변화(선형 합성 + 스케일 누락) | M2 종료 조건에서 기존 명도 비교(캡처 mean 값) |
| 컴포지터 상태 파괴(음수/NaN) | 디코딩·톤매핑에서 음수 클램프 어설션 |
| 16F 스왑체인 대역폭 증가 | HDR 모니터에서만 전환, 캡처·성능 측정 |
| 텍스트 렌더링(Windows 감마 1.8) 변화 | UI를 SDR 흰색 배율 매핑으로 유지해 현재와 동일한 감마 동작 보존 |
| **composition 경로 블로커** | **M2 1차 시도에서 원인 규명. 해결 안 되면 hwnd swapchain으로 폴백(M2 2차)** |

## 검증 방법

- `pwsh scripts/araview-drive.ps1`로 앱 제어(selftest는 SDR 경로 검증용).
- HDR 출력 자체는 SDR 캡처에 나타나지 않으므로: (1) nit 스텝 패턴을 HDR 모니터에서 사람이 확인, (2) `DXGI_OUTPUT_DESC1` 조회 값 기록, (3) SDR 캡처에서는 클리핑 값만 확인.
- Windows 설정: 시스템 > 디스플레이 > HDR 켜기/끄기를 바꿔가며 폴백 확인.

### M2 2차 시도 1단계: 포크와 hwnd 스왑체인 전환 (2026-10-19)

- 포크: `vendor/gpui-pre-windows`(crates.io 0.3.9 원본 복사)를 루트 `Cargo.toml`의 `[patch.crates-io]`로 연결했다.
- 포크 변경: `gpui_windows::set_hdr_output(bool)`을 추가했다. 켜면 플랫폼 초기화에서 `GPUI_DISABLE_DIRECT_COMPOSITION`과 같이 hwnd 스왑체인(`CreateSwapChainForHwnd`)을 쓰고 `WS_EX_NOREDIRECTIONBITMAP`을 빼고 창을 만든다.
- AraView: `platform::prepare_hdr_output`이 GPUI 플랫폼을 만들기 전에 이 함수를 부른다. M4의 설정 토글 전까지는 `ARAVIEW_HDR_OUTPUT=1` 환경 변수로만 켠다.
- 런타임 확인: HDR 모드에서 exstyle `0x40100`(NOREDIRECTIONBITMAP 없음), 화면 캡처와 selftest 21항목 통과. 기본 모드는 exstyle `0x240100`으로 composition 경로를 유지한다.

### M2 2차 시도 2단계: FP16 scRGB 출력 (2026-10-19)

- `vendor/gpui-pre-windows/src/hdr_output.rs`. HDR 모드에서 스왑체인은 `R16G16B16A16_FLOAT` + `SetColorSpace1(DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709)`이다.
- GPUI는 sRGB 인코딩 값을 감마 공간에서 블렌딩하므로, 장면은 기존과 같은 BGRA8 오프스크린 타깃에 그리고 Present 직전 전체 화면 패스 한 번으로 스왑체인에 옮긴다. UI 모양과 블렌딩 결과가 SDR 모드와 같다.
- 변환: 구간별 sRGB EOTF로 선형화한 뒤 SDR 흰색 배율을 곱한다. 배율은 `DisplayConfigGetDeviceInfo(DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL)`의 `SDRWhiteLevel / 1000`(1.0 = 80 nit)이고, 창 모니터를 `viewGdiDeviceName`으로 찾는다. 120프레임마다 다시 읽어 모니터 이동과 설정 변경을 따라간다. 조회 실패 시 1.0.
- 변환 셰이더는 빌드 스크립트(fxc)를 건드리지 않으려고 런타임에 `D3DCompile`로 컴파일한다.
- 한계(3단계에서 해소): 오프스크린이 8비트라 1.0을 넘는 값이 없었다.
- 런타임 확인: 사용자 환경에서 SDR 흰색 배율 3.5(280 nit)로 읽힘, selftest 21항목 통과(전체 화면 전환으로 ResizeBuffers 경로 포함). 화면 캡처는 DWM이 SDR로 되돌린 값이라 HDR 패널에서 SDR 모드와 밝기가 같은지는 눈으로 확인해야 한다.

### M3: 확장 범위 이미지 표시 (2026-10-19)

gpui-pre 코어를 포크하지 않고 Windows 포크와 앱만으로 구현했다.

- 오프스크린: HDR 모드에서 `R16G16B16A16_FLOAT`. 변환 패스는 1.0을 넘는 인코딩 값을 같은 sRGB 곡선으로 이어 선형화한다.
- 톤 매핑: SDR 흰색까지는 그대로, 그 위는 `white + (peak - white) * (1 - exp(-over / (peak - white)))`로 패널 피크에 다가간다. 피크는 스왑체인 출력의 `IDXGIOutput6::GetDesc1().MaxLuminance`(출력 색 공간이 G2084일 때만)이고, 모르면 SDR 흰색에서 자른다.
- 이미지 텍스처: `gpui_windows::register_hdr_image(image_id, w, h, rgba_f16)`로 앱이 `RenderImage` ID별 FP16 픽셀을 등록하면, 아틀라스가 그 이미지를 BGRA8 대신 별도 RGBA16F 텍스처에 올린다. 폴리크롬 스프라이트 셰이더는 값을 그대로 통과시키므로 셰이더 변경은 없다. gpui에는 같은 크기의 SDR `RenderImage`가 그대로 넘어가 레이아웃·캐시·SDR 폴백이 바뀌지 않는다.
- 디코드: `raster_sidecar::decode_linear`가 OpenEXR·Radiance HDR을 `Rgba32F`로 디코드한다(원본 `source_path`, 렌더 경로는 JPEG sidecar라 쓰지 않는다). `picture.rs`가 방향·축소 단계·타일을 SDR과 같게 만들고, 선형 값(1.0 = SDR 흰색)을 sRGB 곡선으로 인코딩해 f16으로 등록한다. 타일이 버려지면 `HdrRegistration`이 등록을 놓는다. 애니메이션과 크기가 다른 렌더 사본은 SDR로 둔다.
- 런타임 확인: 사용자 환경에서 SDR 흰색 280 nit, 피크 400 nit로 읽힘. sample.hdr(32x16)·sample.exr(400x300)이 RGBA16F로 올라감(디버그 로그 `uploading HDR image`). HDR 모드 selftest 21항목 통과. 밝기 띠 패턴(0.5/1.0/1.2/1.4/2/4/8배, `samples/sample-hdr-bands.hdr` 700x300, 확인 당시 1600x900)에서 SDR 흰색 위 띠가 패널에서 더 밝게 구분됨을 사용자가 눈으로 확인했다. 화면 캡처는 80 nit을 넘는 값을 잘라 판정에 쓸 수 없다.
- 남은 일: 노출 조절, 설정 토글(M4), 큰 EXR의 메모리(f32 디코드 + f16 사본) 상한.

