import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { Field, FieldGroup } from "@/components/ui/field"
import { SettingsFieldSet } from "@/components/settings/SettingsFieldSet"
import {
  useSettingsStore,
  type DirSortKey,
  type SettingsState
} from "@/store/settingsStore"
import { applySortSettings } from "@/utils/directoryOptions"
import { ArrowsDownUp, Shuffle } from "@phosphor-icons/react"
import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

export function ListTabPanel() {
  const { t } = useTranslation()
  const settings = useSettingsStore(
    useShallow((state) => ({
      sortKey: state.sortKey,
      sortDescending: state.sortDescending,
      shuffle: state.shuffle,
      includeSubfolders: state.includeSubfolders
    }))
  )

  const handleSortChange = (
    next: Partial<
      Pick<
        SettingsState,
        "sortKey" | "sortDescending" | "shuffle" | "includeSubfolders"
      >
    >
  ) => {
    void applySortSettings(next)
  }

  return (
    <FieldGroup>
      <SettingsFieldSet
        icon={<ArrowsDownUp className="size-6" />}
        title={t("settings.sort.title")}
        description={t("settings.sort.desc")}
      >
        <RadioGroup
          value={settings.shuffle ? "shuffle" : settings.sortKey}
          onValueChange={(value) => {
            if (value === "shuffle") {
              handleSortChange({ shuffle: true })
            } else {
              handleSortChange({
                shuffle: false,
                sortKey: value as DirSortKey
              })
            }
          }}
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="name" id="settings-sort-name" />
            <Label htmlFor="settings-sort-name">
              {t("settings.sort.name")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="date" id="settings-sort-date" />
            <Label htmlFor="settings-sort-date">
              {t("settings.sort.date")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="size" id="settings-sort-size" />
            <Label htmlFor="settings-sort-size">
              {t("settings.sort.size")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="shuffle" id="settings-sort-shuffle" />
            <Label htmlFor="settings-sort-shuffle">
              <span className="flex items-center gap-1">
                <Shuffle className="size-3.5" /> {t("settings.sort.shuffle")}
              </span>
            </Label>
          </Field>
        </RadioGroup>
        <RadioGroup
          value={settings.sortDescending ? "desc" : "asc"}
          onValueChange={(value) =>
            handleSortChange({ sortDescending: value === "desc" })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="asc" id="settings-sort-asc" />
            <Label htmlFor="settings-sort-asc">{t("settings.sort.asc")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="desc" id="settings-sort-desc" />
            <Label htmlFor="settings-sort-desc">
              {t("settings.sort.descOrder")}
            </Label>
          </Field>
        </RadioGroup>
        <Field orientation="horizontal">
          <Switch
            id="settings-subfolders"
            checked={settings.includeSubfolders}
            onCheckedChange={(checked) =>
              handleSortChange({ includeSubfolders: checked === true })
            }
          />
          <Label htmlFor="settings-subfolders">
            {t("settings.sort.subfolders")}
          </Label>
        </Field>
      </SettingsFieldSet>
    </FieldGroup>
  )
}
