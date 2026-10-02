/**
 * 키 입력 대상이 텍스트를 편집하는 요소인지. 전역 단축키가 입력 중인 글자를
 * 가로채지 않도록 확인할 때 쓴다.
 */
export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  if (target.isContentEditable) return true
  return target.closest("input, textarea, select, [contenteditable='true']") !== null
}
