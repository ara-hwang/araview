import { invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect, useState } from "react"

import type { LicenseBundle } from "@/types"
import { errorMessage } from "@/utils/appError"

type LicensesState =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; bundle: LicenseBundle }

/** 동봉된 서드파티 라이선스 자료를 `enabled`가 켜질 때 한 번 읽는다. */
export function useLicenses(enabled: boolean) {
  const [state, setState] = useState<LicensesState>({ status: "loading" })
  const [nonce, setNonce] = useState(0)

  useEffect(() => {
    if (!enabled) return
    let cancelled = false
    setState({ status: "loading" })
    invoke<LicenseBundle>("get_license_bundle")
      .then((bundle) => {
        if (!cancelled) setState({ status: "ready", bundle })
      })
      .catch((error) => {
        if (!cancelled) setState({ status: "error", message: errorMessage(error) })
      })
    return () => {
      cancelled = true
    }
  }, [enabled, nonce])

  const retry = useCallback(() => setNonce((n) => n + 1), [])
  return { state, retry }
}
