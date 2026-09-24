import { WarningCircle } from "@phosphor-icons/react"
import { useTranslation } from "react-i18next"

import { Alert, AlertAction, AlertDescription, AlertTitle } from "@/components/ui/alert"
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
  const { items, loading, pendingExtension, setAssociation, openDefaultAppsSettings } =
    useFileAssociations(active)

  const busy = loading || pendingExtension !== null
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
                data-disabled={pending || undefined}
              >
                <FieldContent>
                  <FieldLabel htmlFor={fieldId}>.{item.extension}</FieldLabel>
                  <FieldDescription>{extensionLabel(item.extension)}</FieldDescription>
                </FieldContent>
                <Switch
                  id={fieldId}
                  checked={item.associated}
                  disabled={pending}
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
          <AlertAction>
            <Button variant="outline" size="sm" onClick={() => void openDefaultAppsSettings()}>
              {t("settings.ext.openSettings")}
            </Button>
          </AlertAction>
        </Alert>
      ) : null}

      <div className="flex flex-col gap-3 border-t pt-4">
        <div className="flex flex-col gap-1">
          <h3 className="text-base font-medium">{t("settings.thumb.title")}</h3>
          <p className="text-sm text-muted-foreground">{t("settings.thumb.desc")}</p>
        </div>
        <Field orientation="horizontal" data-disabled={thumbBusy || undefined}>
          <FieldContent>
            <FieldLabel htmlFor="settings-thumb-psd">{t("settings.thumb.label")}</FieldLabel>
            <FieldDescription role="status">
              {thumbStatus !== null && !thumbStatus.dll_exists
                ? t("settings.thumb.dllMissing")
                : t("settings.thumb.note")}
            </FieldDescription>
          </FieldContent>
          <Switch
            id="settings-thumb-psd"
            checked={thumbStatus?.registered ?? false}
            disabled={thumbBusy}
            aria-busy={thumbLoading || undefined}
            onCheckedChange={(checked) => {
              void setThumbnail(checked)
            }}
          />
        </Field>
      </div>
    </div>
  )
}
