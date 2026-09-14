// 개발 실행(npm run tauri dev)은 설치 버전과 확장자 연결이 섞이지 않도록
// 별도 이름("AraView (Dev)")으로 표시/등록한다.
// 백엔드 file_assoc.rs의 APP_NAME 규칙과 동일하게 유지할 것.
export const DEV_BUILD_SUFFIX = import.meta.env.DEV ? " (Dev)" : ""

/** Windows 기본 앱/확장자 연결 목록에 등록되는 앱 이름. */
export const APP_DISPLAY_NAME = `AraView${DEV_BUILD_SUFFIX}`
