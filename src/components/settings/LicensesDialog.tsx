import { useMemo, useState } from "react"
import { useTranslation } from "react-i18next"

import { LicenseList, type LicenseListItem } from "@/components/settings/LicenseList"
import { LicenseNotice } from "@/components/settings/LicenseNotice"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Spinner } from "@/components/ui/spinner"
import { useLicenses } from "@/hooks/useLicenses"
import type { LicenseBundle } from "@/types"

type LicensesDialogProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
}

const NOTICE_FILE = "THIRD_PARTY_LICENSES.md"

const docKey = (name: string) => `doc:${name}`
const pkgKey = (index: number) => `pkg:${index}`

const preClass =
  "p-4 font-mono text-xs leading-relaxed break-words whitespace-pre-wrap select-text focus-visible:outline-2 focus-visible:outline-ring"

function LicensesBody({ bundle }: { bundle: LicenseBundle }) {
  const { t } = useTranslation()
  const [selectedKey, setSelectedKey] = useState(() => docKey(bundle.documents[0].name))

  const items = useMemo<LicenseListItem[]>(() => {
    const docs = bundle.documents.map((doc) => ({
      key: docKey(doc.name),
      title:
        doc.name === NOTICE_FILE
          ? t("settings.licenses.notice")
          : doc.name.replace(/-copyright\.txt$/, ""),
      subtitle: t("settings.licenses.documentsGroup")
    }))
    const packages = bundle.packages.map((pkg, index) => ({
      key: pkgKey(index),
      title: `${pkg.name} ${pkg.version}`,
      subtitle: `${pkg.ecosystem === "npm" ? "npm" : "Rust"} · ${pkg.license}`
    }))
    return [...docs, ...packages]
  }, [bundle, t])

  let detail: React.ReactNode = null
  if (selectedKey.startsWith("doc:")) {
    const doc = bundle.documents.find((d) => docKey(d.name) === selectedKey)
    if (doc?.name === NOTICE_FILE) detail = <LicenseNotice content={doc.content} />
    else if (doc) {
      detail = (
        <pre tabIndex={0} className={preClass}>
          {doc.content}
        </pre>
      )
    }
  } else {
    const pkg = bundle.packages[Number(selectedKey.slice(4))]
    if (pkg) {
      detail = (
        <div className="flex flex-col gap-3 p-4 select-text">
          <div>
            <h3 className="text-base font-semibold">
              {pkg.name} {pkg.version}
            </h3>
            <p className="text-sm text-muted-foreground">
              {t("settings.licenses.declared")}: {pkg.license}
            </p>
          </div>
          {pkg.templated && (
            <p className="rounded-lg border bg-muted/30 px-3 py-2 text-sm">
              {t("settings.licenses.templatedNote")}
            </p>
          )}
          {pkg.files.length === 0 ? (
            <p className="rounded-lg border bg-muted/30 px-3 py-2 text-sm text-muted-foreground">
              {t("settings.licenses.noLicenseFile")}
            </p>
          ) : (
            pkg.files.map((file) => (
              <section key={file.name} className="flex flex-col gap-1">
                {pkg.files.length > 1 && (
                  <h4 className="text-xs font-medium text-muted-foreground">{file.name}</h4>
                )}
                <pre
                  tabIndex={0}
                  className="rounded-lg border bg-muted/30 font-mono text-xs leading-relaxed break-words whitespace-pre-wrap focus-visible:outline-2 focus-visible:outline-ring"
                >
                  <span className="block p-3">{bundle.texts[file.id] ?? ""}</span>
                </pre>
              </section>
            ))
          )}
        </div>
      )
    }
  }

  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex w-72 shrink-0 flex-col border-r">
        <LicenseList
          items={items}
          selectedKey={selectedKey}
          onSelect={setSelectedKey}
          label={t("settings.licenses.listLabel")}
        />
      </div>
      {/* key: 항목을 바꾸면 스크롤을 맨 위로 되돌린다. */}
      <ScrollArea key={selectedKey} className="min-h-0 min-w-0 flex-1">
        {detail}
      </ScrollArea>
    </div>
  )
}

export function LicensesDialog({ open, onOpenChange }: LicensesDialogProps) {
  const { t } = useTranslation()
  const { state, retry } = useLicenses(open)

  return (
    // modal="trap-focus": 설정 대화상자와 같은 이유로 캡션 버튼을 막지 않는다.
    <Dialog open={open} onOpenChange={onOpenChange} modal="trap-focus">
      <DialogContent className="w-[920px] max-w-[calc(100vw-2rem)] gap-0 overflow-hidden p-0 no-drag sm:max-w-[calc(100vw-2rem)]">
        <div className="flex h-[min(640px,86vh)] flex-col">
          <div className="border-b px-4 py-3 pr-12">
            <DialogTitle>{t("settings.licenses.title")}</DialogTitle>
            <DialogDescription className="sr-only">{t("settings.licenses.desc")}</DialogDescription>
          </div>

          {state.status === "loading" && (
            <div className="flex flex-1 items-center justify-center gap-2 text-sm text-muted-foreground">
              <Spinner />
              {t("settings.licenses.loading")}
            </div>
          )}

          {state.status === "error" && (
            <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6 text-center">
              <p className="text-sm font-medium">{t("settings.licenses.loadFail")}</p>
              <p className="text-sm text-muted-foreground">{state.message}</p>
              <Button variant="outline" onClick={retry}>
                {t("settings.licenses.retry")}
              </Button>
            </div>
          )}

          {state.status === "ready" && <LicensesBody bundle={state.bundle} />}
        </div>
      </DialogContent>
    </Dialog>
  )
}
