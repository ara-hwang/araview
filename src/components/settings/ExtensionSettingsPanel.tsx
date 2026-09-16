import { WarningCircle } from "@phosphor-icons/react"
import { useTranslation } from "react-i18next"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel
} from "@/components/ui/field"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { APP_DISPLAY_NAME } from "@/constants/app"
import { extensionLabel } from "@/constants/extensionLabels"
import { hasBlockedAssociation, useFileAssociations } from "@/hooks/useFileAssociations"
import { usePsdThumbnail } from "@/hooks/usePsdThumbnail"

type ExtensionSettingsPanelProps = {
  active: boolean
}

export function ExtensionSettingsPanel({ active }: ExtensionSettingsPanelProps) {
  const { t } = useTranslation()
  const {
    items,
    loading,
    pendingExtension,
    pendingAll,
    setAssociation,
    setAllAssociations,
    openDefaultAppsSettings
  } = useFileAssociations(active)

  const busy = loading || pendingAll || pendingExtension !== null
  const blocked = hasBlockedAssociation(items)
  const {
    status: thumbStatus,
    loading: thumbLoading,
    pending: thumbPending,
    setThumbnail
  } = usePsdThumbnail(active)
  const thumbBusy = busy || thumbLoading || thumbPending

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-1">
        <h3 className="text-base font-medium">{t("settings.ext.title")}</h3>
        <p className="text-sm text-muted-foreground">{t("settings.ext.desc")}</p>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => void setAllAssociations(true)}
        >
          {pendingAll ? <Spinner data-icon="inline-start" /> : null}
          {t("settings.ext.connectAll")}
        </Button>
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => void setAllAssociations(false)}
        >
          {t("settings.ext.disconnectAll")}
        </Button>
      </div>

      {loading && items.length === 0 ? (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Spinner />
          {t("settings.ext.loading")}
        </div>
      ) : (
        <FieldGroup className="gap-3">
          {items.map((item) => {
            const fieldId = `settings-ext-${item.extension}`
            const pending = pendingExtension === item.extension
            return (
              <Field
                key={item.extension}
                orientation="horizontal"
                data-disabled={pendingAll || pending || undefined}
              >
                <FieldContent>
                  <FieldLabel htmlFor={fieldId}>.{item.extension}</FieldLabel>
                  <FieldDescription>{extensionLabel(item.extension)}</FieldDescription>
                </FieldContent>
                <Switch
                  id={fieldId}
                  checked={item.associated}
                  disabled={pendingAll || pending}
                  onCheckedChange={(checked) => {
                    void setAssociation(item.extension, checked)
                  }}
                />
              </Field>
            )
          })}
        </FieldGroup>
      )}

      {blocked ? (
        <Alert>
          <WarningCircle className="size-6" />
          <AlertTitle>{t("settings.ext.blockedTitle")}</AlertTitle>
          <AlertDescription>
            {t("settings.ext.blockedDesc", { appName: APP_DISPLAY_NAME })}
          </AlertDescription>
        </Alert>
      ) : null}

      <Button
        variant="outline"
        size="sm"
        className="self-start"
        onClick={() => void openDefaultAppsSettings()}
      >
        {t("settings.ext.openSettings")}
      </Button>

      <div className="flex flex-col gap-2 border-t pt-4">
        <h3 className="text-base font-medium">{t("settings.thumb.title")}</h3>
        <p className="text-sm text-muted-foreground">{t("settings.thumb.desc")}</p>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            disabled={thumbBusy}
            onClick={() => void setThumbnail(!(thumbStatus?.registered ?? false))}
          >
            {thumbPending ? <Spinner data-icon="inline-start" /> : null}
            {thumbStatus?.registered ? t("settings.thumb.disable") : t("settings.thumb.enable")}
          </Button>
          <span className="text-sm text-muted-foreground" role="status">
            {thumbLoading || thumbStatus === null
              ? t("settings.thumb.checking")
              : thumbStatus.registered
                ? t("settings.thumb.enabled")
                : t("settings.thumb.disabled")}
          </span>
        </div>
        {thumbStatus !== null && !thumbStatus.dll_exists ? (
          <p className="text-sm text-muted-foreground">{t("settings.thumb.dllMissing")}</p>
        ) : null}
        <p className="text-sm text-muted-foreground">{t("settings.thumb.note")}</p>
      </div>
    </div>
  )
}
