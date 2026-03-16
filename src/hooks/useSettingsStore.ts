import { useEffect, useState } from "react";
import type { Settings } from "../types";
import { getSettings, subscribeToSettings } from "../store/appStore";

export const useSettingsStore = () => {
  const [settings, setSettings] = useState<Settings>(() => getSettings());

  useEffect(() => {
    const subscription = subscribeToSettings((next) => {
      setSettings(next);
    });

    return () => {
      subscription.unsubscribe();
    };
  }, []);

  return settings;
};
