import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import type { Settings, CacheMode } from "@/types/settings";
import { updateSettings, useSettingsStore } from "@/store/settingsStore";
import { HardDriveIcon, NavigationIcon, ArrowLeft } from "lucide-react";
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLegend,
  FieldSeparator,
  FieldSet,
} from "@/components/ui/field";
import { getApp } from "@/store/appStore";

export const Route = createFileRoute("/settings/")({
  component: SettingsPage,
  staticData: {
    headerLeftSlot: <SettingsHeaderSlot />,
  },
});

function SettingsPage() {
  const settings = useSettingsStore();

  const handleSettingsChange = (next: Partial<Settings>) => {
    void updateSettings(next);
  };

  return (
    <div className="flex flex-1 flex-col">
      {/* Content */}
      <div className="flex-1 overflow-y-auto p-6">
        <FieldGroup>
          {/* Navigation */}
          <CustomFieldSet
            icon={<NavigationIcon className="size-4" />}
            title="Navigation"
            description="Navigation mode for the image viewer"
          >
            <RadioGroup
              value={settings.loopNavigation ? "loop" : "stop"}
              onValueChange={(value) =>
                handleSettingsChange({ loopNavigation: value === "loop" })
              }
            >
              <Field orientation="horizontal">
                <RadioGroupItem value="stop" id="stop" />
                <Label htmlFor="stop">Stop at first / last image</Label>
              </Field>
              <Field orientation="horizontal">
                <RadioGroupItem value="loop" id="loop" />
                <Label htmlFor="loop">Loop through images</Label>
              </Field>
            </RadioGroup>
          </CustomFieldSet>

          <FieldSeparator />

          {/* Cache */}
          <CustomFieldSet
            icon={<HardDriveIcon className="size-4" />}
            title="Cache"
            description="Cache mode for the image viewer"
          >
            <RadioGroup
              value={settings.cacheMode}
              onValueChange={(value) =>
                handleSettingsChange({ cacheMode: value as CacheMode })
              }
            >
              <Field orientation="horizontal">
                <RadioGroupItem value="off" id="off" />
                <Label htmlFor="off">Off (current image only)</Label>
              </Field>
              <Field orientation="horizontal">
                <RadioGroupItem value="nearby" id="nearby" />
                <Label htmlFor="nearby">Nearby (previous / next preload)</Label>
              </Field>
              <Field orientation="horizontal">
                <RadioGroupItem value="extended" id="extended" />
                <Label htmlFor="extended">Extended (up to ±3 preload)</Label>
              </Field>
            </RadioGroup>
          </CustomFieldSet>
        </FieldGroup>
      </div>
    </div>
  );
}

function SettingsHeaderSlot() {
  const navigate = useNavigate();

  const handleBack = () => {
    void navigate({ to: getApp().imageInfo ? "/image" : "/" });
  };

  return (
    <div
      className="flex items-center gap-2"
      style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
    >
      <Button
        variant="outline"
        size="icon"
        onClick={handleBack}
        title="Back"
        style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
      >
        <ArrowLeft />
      </Button>
      <span className="text-sm font-medium">Settings</span>
    </div>
  );
}

function CustomFieldSet(props: {
  icon: React.ReactNode;
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <FieldSet>
      <FieldLegend className="flex items-center gap-2">
        {props.icon}
        {props.title}
      </FieldLegend>
      <FieldDescription>{props.description}</FieldDescription>
      {props.children}
    </FieldSet>
  );
}
