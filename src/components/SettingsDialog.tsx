import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import type { Settings, CacheMode } from "@/types/settings";
import { updateSettings, useSettingsStore } from "@/store/settingsStore";

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
              <RadioGroupItemContainer>
                <RadioGroupItem value="stop" id="stop" />
                <Label htmlFor="stop">Stop at first / last image</Label>
              </RadioGroupItemContainer>
              <RadioGroupItemContainer>
                <RadioGroupItem value="loop" id="loop" />
                <Label htmlFor="loop">Loop through images</Label>
              </RadioGroupItemContainer>
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
              <RadioGroupItemContainer>
                <RadioGroupItem value="off" id="off" />
                <Label htmlFor="off">Off (current image only)</Label>
              </RadioGroupItemContainer>
              <RadioGroupItemContainer>
                <RadioGroupItem value="nearby" id="nearby" />
                <Label htmlFor="nearby">Nearby (previous / next preload)</Label>
              </RadioGroupItemContainer>
              <RadioGroupItemContainer>
                <RadioGroupItem value="extended" id="extended" />
                <Label htmlFor="extended">Extended (up to ±3 preload)</Label>
              </RadioGroupItemContainer>
            </RadioGroup>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function RadioGroupItemContainer(props: { children: React.ReactNode }) {
  return <div className="flex items-center gap-3">{props.children}</div>;
}
