import { useNavigate } from "@tanstack/react-router"
import { useCallback } from "react"

import { closeImage } from "@/store/appStore"

/** 열린 이미지를 닫고 홈(/)으로 이동한다. 상태 초기화가 선행되어야 홈에서 되돌아오지 않는다. */
export function useCloseImage() {
  const navigate = useNavigate()

  return useCallback(() => {
    closeImage()
    void navigate({ to: "/" })
  }, [navigate])
}
