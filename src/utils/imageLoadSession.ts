let loadGeneration = 0

/** 새 이미지 로드 시퀀스를 시작하고 토큰을 반환한다. */
export function beginImageLoad(): number {
  loadGeneration += 1
  return loadGeneration
}

export function isCurrentImageLoad(token: number): boolean {
  return token === loadGeneration
}

/** 테스트 전용 */
export function resetImageLoadSession(): void {
  loadGeneration = 0
}
