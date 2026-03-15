/**
 * 컨테이너·이미지 크기와 줌으로 이동 가능한 최대 오프셋(절반)을 계산합니다.
 */
export function getPositionBounds(
  containerW: number,
  containerH: number,
  imgW: number,
  imgH: number,
  zoom: number,
): { maxX: number; maxY: number } {
  const scaledW = imgW * zoom;
  const scaledH = imgH * zoom;
  return {
    maxX: scaledW >= containerW ? (scaledW - containerW) / 2 : 0,
    maxY: scaledH >= containerH ? (scaledH - containerH) / 2 : 0,
  };
}

/**
 * 위치를 maxX, maxY 기준으로 클램프합니다.
 */
export function clampPosition(
  x: number,
  y: number,
  maxX: number,
  maxY: number,
): { x: number; y: number } {
  return {
    x: maxX > 0 ? Math.max(-maxX, Math.min(maxX, x)) : 0,
    y: maxY > 0 ? Math.max(-maxY, Math.min(maxY, y)) : 0,
  };
}

/**
 * 이미지가 컨테이너에 완전히 들어가는지 여부
 */
export function isFullyContained(
  containerW: number,
  containerH: number,
  imgW: number,
  imgH: number,
  zoom: number,
): boolean {
  const scaledW = imgW * zoom;
  const scaledH = imgH * zoom;
  return scaledW <= containerW && scaledH <= containerH;
}
