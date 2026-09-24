import {
  ArrowClockwise,
  FileArchive,
  FileImage,
  HardDrive,
  Trash,
  WarningCircle
} from "@phosphor-icons/react"
import { confirm } from "@tauri-apps/plugin-dialog"
import { useCallback } from "react"
import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

import { SettingsFieldSet } from "@/components/settings/SettingsFieldSet"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldSeparator,
  FieldTitle
} from "@/components/ui/field"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { toast } from "@/components/ui/toast"
import { useCacheManager } from "@/hooks/useCacheManager"
import { updateSettings, useSettingsStore } from "@/store/settingsStore"
import type { CacheCategoryKey, CacheScope } from "@/types"
import { formatFileSize } from "@/utils/format"

function CategoryIcon({ category }: { category: CacheCategoryKey }) {
  if (category === "archives") return <FileArchive aria-hidden="true" />
  if (category === "other") return <HardDrive aria-hidden="true" />
  return <FileImage aria-hidden="true" />
}

export function CacheManagementPanel() {
  const { t } = useTranslation()
  const tx = t as unknown as (key: string) => string
  const { stats, loading, clearingScope, error, refresh, clear } = useCacheManager()
  const { cacheStorageMode, locale } = useSettingsStore(
    useShallow((state) => ({
      cacheStorageMode: state.cacheStorageMode,
      locale: state.language
    }))
  )

  const handleStorageChange = useCallback(
    async (checked: boolean) => {
      if (!checked) {
        const ok = await confirm(t("confirm.cacheStorage.disable.message"), {
          title: t("confirm.cacheStorage.disable.title"),
          kind: "warning"
        }).catch(() => false)
        if (!ok) return
      }
      const saved = await updateSettings({ cacheStorageMode: checked ? "persistent" : "temporary" })
      if (saved) {
        toast.info(t("toast.cache.storagePending"))
      }
    },
    [t]
  )

  const handleClear = useCallback(
    async (scope: CacheScope) => {
      if (scope === "all") {
        const ok = await confirm(t("confirm.cache.clearAll.message"), {
          title: t("confirm.cache.clearAll.title"),
          kind: "warning"
        }).catch(() => false)
        if (!ok) return
      }
      try {
        const result = await clear(scope)
        if (result.removed_bytes === 0 && result.stats.protected_file_count > 0) {
          toast.info(t("toast.cache.protectedOnly"))
        } else {
          toast.success(
            t("toast.cache.cleared", {
              size: formatFileSize(result.removed_bytes, locale)
            })
          )
        }
        if (result.failed_file_count > 0) {
          toast.info(t("toast.cache.partial"))
        }
      } catch {
        // The hook exposes the structured error in the panel.
      }
    },
    [clear, locale, t]
  )

  const pendingRestart = stats !== null && stats.storage_mode !== cacheStorageMode
  const categories =
    stats?.categories.filter((category) => category.key !== "other" || category.file_count > 0) ??
    []

  return (
    <>
      <SettingsFieldSet
        icon={<HardDrive className="size-6" />}
        title={t("settings.cacheStorage.title")}
        description={t("settings.cacheStorage.desc")}
      >
        <Field orientation="horizontal">
          <FieldContent>
            <FieldTitle>{t("settings.cacheStorage.persistent")}</FieldTitle>
            <FieldDescription>
              {cacheStorageMode === "persistent"
                ? t("settings.cacheStorage.persistentDesc")
                : t("settings.cacheStorage.temporaryDesc")}
            </FieldDescription>
          </FieldContent>
          <Switch
            checked={cacheStorageMode === "persistent"}
            onCheckedChange={(checked) => void handleStorageChange(checked)}
            aria-label={t("settings.cacheStorage.toggle")}
          />
        </Field>
        {pendingRestart ? (
          <Alert>
            <WarningCircle aria-hidden="true" />
            <AlertTitle>{t("settings.cacheStorage.pendingTitle")}</AlertTitle>
            <AlertDescription>{t("settings.cacheStorage.pendingDesc")}</AlertDescription>
          </Alert>
        ) : null}
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<Trash className="size-6" />}
        title={t("settings.cacheManagement.title")}
        description={t("settings.cacheManagement.desc")}
      >
        <div className="flex justify-end">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void refresh()}
            disabled={loading || clearingScope !== null}
          >
            {loading ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <ArrowClockwise data-icon="inline-start" />
            )}
            {t("settings.cacheManagement.refresh")}
          </Button>
        </div>

        {stats && !stats.persistent_available ? (
          <Alert>
            <WarningCircle aria-hidden="true" />
            <AlertTitle>{t("settings.cacheManagement.persistentUnavailable")}</AlertTitle>
          </Alert>
        ) : null}

        {error ? (
          <Alert variant="destructive">
            <WarningCircle aria-hidden="true" />
            <AlertTitle>{t("settings.cacheManagement.errorTitle")}</AlertTitle>
            <AlertDescription>
              {error.kind === "unknown"
                ? t("settings.cacheManagement.errorDescription")
                : tx(error.hintKey)}
            </AlertDescription>
            <div className="mt-2">
              <Button variant="outline" size="sm" onClick={() => void refresh()}>
                {t("settings.cacheManagement.retry")}
              </Button>
            </div>
          </Alert>
        ) : null}

        {loading && !stats ? (
          <div className="flex items-center gap-2 py-4 text-sm text-muted-foreground" role="status">
            <Spinner />
            {t("settings.cacheManagement.loading")}
          </div>
        ) : null}

        {stats && stats.total_bytes === 0 && !loading && !error ? (
          <Empty className="min-h-40">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HardDrive />
              </EmptyMedia>
              <EmptyTitle>{t("settings.cacheManagement.emptyTitle")}</EmptyTitle>
              <EmptyDescription>{t("settings.cacheManagement.emptyDesc")}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : null}

        {stats && stats.total_bytes > 0 ? (
          <>
            <div className="rounded-lg border bg-muted/30 p-3" aria-live="polite">
              <div className="flex items-baseline justify-between gap-3">
                <span className="text-sm text-muted-foreground">
                  {t("settings.cacheManagement.total")}
                </span>
                <span className="text-sm font-medium tabular-nums">
                  {formatFileSize(stats.total_bytes, locale)}
                </span>
              </div>
              <div className="mt-1 text-xs text-muted-foreground">
                {t(
                  stats.storage_mode === "persistent"
                    ? "settings.cacheManagement.persistentActive"
                    : "settings.cacheManagement.temporaryActive"
                )}
              </div>
              <div className="mt-1 text-xs text-muted-foreground">
                {t("settings.cacheManagement.files", { count: stats.file_count })}
                {stats.protected_file_count > 0
                  ? ` · ${t("settings.cacheManagement.protected", {
                      count: stats.protected_file_count
                    })}`
                  : ""}
              </div>
              <div className="mt-1 text-xs text-muted-foreground">
                {t("settings.cacheManagement.limit", {
                  size: formatFileSize(stats.total_limit_bytes, locale)
                })}
              </div>
            </div>

            <div className="flex flex-col">
              {categories.map((category) => {
                const busy = clearingScope === category.key
                return (
                  <div key={category.key} className="border-b py-3 last:border-b-0">
                    <Field orientation="responsive">
                      <FieldContent>
                        <FieldTitle>
                          <span className="flex items-center gap-2">
                            <CategoryIcon category={category.key} />
                            {t(`settings.cacheManagement.categories.${category.key}`)}
                          </span>
                        </FieldTitle>
                        <FieldDescription>
                          {t("settings.cacheManagement.categorySize", {
                            size: formatFileSize(category.bytes, locale),
                            count: category.file_count
                          })}
                          {category.protected_file_count > 0
                            ? ` · ${t("settings.cacheManagement.protected", {
                                count: category.protected_file_count
                              })}`
                            : ""}
                        </FieldDescription>
                      </FieldContent>
                      <Button
                        variant="destructive"
                        size="sm"
                        disabled={clearingScope !== null || category.file_count === 0}
                        aria-label={t("settings.cacheManagement.clearCategory", {
                          name: t(`settings.cacheManagement.categories.${category.key}`)
                        })}
                        onClick={() => void handleClear(category.key)}
                      >
                        {busy ? (
                          <Spinner data-icon="inline-start" />
                        ) : (
                          <Trash data-icon="inline-start" />
                        )}
                        {t("settings.cacheManagement.clear")}
                      </Button>
                    </Field>
                  </div>
                )
              })}
            </div>

            <Button
              variant="destructive"
              onClick={() => void handleClear("all")}
              disabled={clearingScope !== null}
            >
              {clearingScope === "all" ? (
                <Spinner data-icon="inline-start" />
              ) : (
                <Trash data-icon="inline-start" />
              )}
              {t("settings.cacheManagement.clearAll")}
            </Button>
          </>
        ) : null}
      </SettingsFieldSet>
    </>
  )
}
