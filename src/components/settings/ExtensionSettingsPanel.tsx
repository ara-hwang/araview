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
import { extensionLabel } from "@/constants/extensionLabels"
import {
  hasBlockedAssociation,
  useFileAssociations
} from "@/hooks/useFileAssociations"
import { CircleAlertIcon } from "lucide-react"

type ExtensionSettingsPanelProps = {
  active: boolean
}

export function ExtensionSettingsPanel({
  active
}: ExtensionSettingsPanelProps) {
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

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-1">
        <h3 className="text-base font-medium">확장자 연결</h3>
        <p className="text-muted-foreground text-sm">
          이 앱을 해당 파일 형식의 기본 프로그램으로 연결합니다.
        </p>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => void setAllAssociations(true)}
        >
          {pendingAll ? <Spinner data-icon="inline-start" /> : null}
          모두 연결
        </Button>
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => void setAllAssociations(false)}
        >
          모두 해제
        </Button>
      </div>

      {loading && items.length === 0 ? (
        <div className="text-muted-foreground flex items-center gap-2 text-sm">
          <Spinner />
          연결 상태를 확인하는 중
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
                  <FieldDescription>
                    {extensionLabel(item.extension)}
                    {item.needs_os_confirmation && !item.associated
                      ? " · Windows 기본 앱 확인 필요"
                      : null}
                  </FieldDescription>
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
          <CircleAlertIcon />
          <AlertTitle>Windows에서 기본 앱을 확인해야 합니다</AlertTitle>
          <AlertDescription>
            Windows 10/11은 일부 확장자의 기본 앱 변경을 OS 설정에서만
            허용합니다. 연결을 켠 뒤에도 적용되지 않으면 Windows 설정에서 Image
            Viewer를 선택하세요.
          </AlertDescription>
        </Alert>
      ) : null}

      <Button
        variant="outline"
        size="sm"
        className="self-start"
        onClick={() => void openDefaultAppsSettings()}
      >
        Windows 기본 앱 설정 열기
      </Button>
    </div>
  )
}
