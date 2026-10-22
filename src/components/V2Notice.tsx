import { useState } from "react"
import { useTranslation } from "react-i18next"

import { ExternalLink } from "@/components/ExternalLink"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from "@/components/ui/dialog"

const RELEASES_URL = "https://github.com/ara-hwang/araview/releases/latest"
const SEEN_KEY = "araview:v2-notice-seen"

function alreadySeen(): boolean {
  try {
    return localStorage.getItem(SEEN_KEY) === "1"
  } catch {
    return false
  }
}

function markSeen() {
  try {
    localStorage.setItem(SEEN_KEY, "1")
  } catch {
    // 저장소를 못 쓰면 다음 실행에 한 번 더 보일 뿐이다.
  }
}

/**
 * 2.x는 이 앱의 자동 업데이트로 받을 수 없어서, 처음 한 번만 직접 설치하라고 안내한다.
 * 네트워크는 쓰지 않는다(오프라인 우선 원칙).
 */
export function V2Notice() {
  const { t } = useTranslation()
  const [open, setOpen] = useState(() => !alreadySeen())

  const close = () => {
    markSeen()
    setOpen(false)
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !next && close()} modal="trap-focus">
      <DialogContent data-testid="v2-notice">
        <DialogHeader>
          <DialogTitle>{t("dialog.update.v2NoticeTitle")}</DialogTitle>
          <DialogDescription>{t("dialog.update.v2NoticeDesc")}</DialogDescription>
        </DialogHeader>
        <p className="text-sm">
          <ExternalLink href={RELEASES_URL}>{t("dialog.update.v2NoticeRelease")}</ExternalLink>
        </p>
        <DialogFooter>
          <Button data-testid="v2-notice-close" onClick={close}>
            {t("dialog.update.v2NoticeClose")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
