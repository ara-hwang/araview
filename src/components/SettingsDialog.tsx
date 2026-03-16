import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import type { Settings, BackgroundType, CacheMode } from "../types";
import { useSettingsStore } from "../hooks/useSettingsStore";
import { updateSettings } from "../store/appStore";

type SettingsDialogProps = {
  open: boolean;
  onClose: () => void;
};

export function SettingsDialog({ open, onClose }: SettingsDialogProps) {
  const settings = useSettingsStore();

  const handleSettingsChange = (next: Partial<Settings>) => {
    void updateSettings(next);
  };

  return (
    <Dialog open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
      <DialogContent>
        <DialogTitle>Settings</DialogTitle>

        <div className="space-y-5">
          {/* Background */}
          <div>
            <p className="text-lg font-medium text-[hsl(var(--foreground))] mb-2">
              Background
            </p>
            <RadioGroup
              value={settings.background}
              onValueChange={(value) =>
                handleSettingsChange({ background: value as BackgroundType })
              }
            >
              <div className="flex items-center gap-3">
                <RadioGroupItem value="checkered" id="checkered" />
                <Label htmlFor="checkered">Checkered</Label>
              </div>
              <div className="flex items-center gap-3">
                <RadioGroupItem value="solid" id="solid" />
                <Label htmlFor="solid">Solid color</Label>
              </div>
            </RadioGroup>
          </div>

          <div>
            <p className="text-lg font-medium text-[hsl(var(--foreground))] mb-2">
              Navigation
            </p>
            <RadioGroup
              value={settings.loopNavigation ? "loop" : "stop"}
              onValueChange={(value) =>
                handleSettingsChange({ loopNavigation: value === "loop" })
              }
            >
              <div className="flex items-center gap-3">
                <RadioGroupItem value="stop" id="stop" />
                <Label htmlFor="stop">Stop at first / last image</Label>
              </div>
              <div className="flex items-center gap-3">
                <RadioGroupItem value="loop" id="loop" />
                <Label htmlFor="loop">Loop through images</Label>
              </div>
            </RadioGroup>
          </div>

          {/* Cache */}
          <div>
            <p className="text-lg font-medium text-[hsl(var(--foreground))] mb-2">
              Cache
            </p>
            <RadioGroup
              value={settings.cacheMode}
              onValueChange={(value) =>
                handleSettingsChange({ cacheMode: value as CacheMode })
              }
            >
              <div className="flex items-center gap-3">
                <RadioGroupItem value="off" id="off" />
                <Label htmlFor="off">Off (current image only)</Label>
              </div>
              <div className="flex items-center gap-3">
                <RadioGroupItem value="nearby" id="nearby" />
                <Label htmlFor="nearby">Nearby (previous / next preload)</Label>
              </div>
              <div className="flex items-center gap-3">
                <RadioGroupItem value="extended" id="extended" />
                <Label htmlFor="extended">Extended (up to ±3 preload)</Label>
              </div>
            </RadioGroup>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
