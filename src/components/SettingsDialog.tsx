import { Dialog } from "@base-ui/react";
import { X } from "lucide-react";
import { RadioGroupRoot, RadioGroupItem } from "@/components/ui/radio-group";
import { Button } from "@/components/ui/button";
import type { Settings, BackgroundType } from "../types";

interface SettingsDialogProps {
  open: boolean;
  settings: Settings;
  onClose: () => void;
  onSettingsChange: (settings: Partial<Settings>) => void;
}

export function SettingsDialog({
  open,
  settings,
  onClose,
  onSettingsChange,
}: SettingsDialogProps) {
  return (
    <Dialog.Root open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
      <Dialog.Portal>
        <Dialog.Backdrop className="fixed inset-0 bg-black/50 z-40" />
        <Dialog.Popup className="fixed z-50 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-80 rounded-lg bg-[hsl(var(--card))] border border-[hsl(var(--border))] shadow-xl p-5">
          <div className="flex items-center justify-between mb-4">
            <Dialog.Title className="text-base font-semibold text-[hsl(var(--foreground))]">
              Settings
            </Dialog.Title>
            <Button variant="ghost" size="icon" onClick={onClose} title="Close">
              <X />
            </Button>
          </div>

          <div className="space-y-5">
            {/* Background */}
            <div>
              <p className="text-sm font-medium text-[hsl(var(--foreground))] mb-2">
                Background
              </p>
              <RadioGroupRoot
                value={settings.background}
                onValueChange={(value) =>
                  onSettingsChange({ background: value as BackgroundType })
                }
              >
                <RadioGroupItem value="checkered">
                  <span className="text-sm text-[hsl(var(--foreground))]">Checkered</span>
                </RadioGroupItem>
                <RadioGroupItem value="solid">
                  <span className="text-sm text-[hsl(var(--foreground))]">Solid color</span>
                </RadioGroupItem>
              </RadioGroupRoot>
            </div>

            {/* Navigation */}
            <div>
              <p className="text-sm font-medium text-[hsl(var(--foreground))] mb-2">
                Navigation
              </p>
              <RadioGroupRoot
                value={settings.loopNavigation ? "loop" : "stop"}
                onValueChange={(value) =>
                  onSettingsChange({ loopNavigation: value === "loop" })
                }
              >
                <RadioGroupItem value="stop">
                  <span className="text-sm text-[hsl(var(--foreground))]">Stop at first / last image</span>
                </RadioGroupItem>
                <RadioGroupItem value="loop">
                  <span className="text-sm text-[hsl(var(--foreground))]">Loop through images</span>
                </RadioGroupItem>
              </RadioGroupRoot>
            </div>
          </div>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
