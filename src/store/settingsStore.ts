import { useEffect, useState } from "react";
import { createStore } from "@tanstack/store";
import { Store as TauriStore } from "@tauri-apps/plugin-store";
import type { Settings } from "../types";

const DEFAULT_SETTINGS: Settings = {
  background: "checkered",
  loopNavigation: false,
  cacheMode: "nearby",
};

const settingsStore = createStore<{ settings: Settings }>({
  settings: DEFAULT_SETTINGS,
});

let tauriStorePromise: Promise<TauriStore> | null = null;

const getTauriStore = (): Promise<TauriStore> => {
  if (!tauriStorePromise) {
    tauriStorePromise = TauriStore.load("settings.json");
  }
  return tauriStorePromise;
};

export const getSettings = () => settingsStore.state.settings;

export const subscribeToSettings = (listener: (settings: Settings) => void) => {
  return settingsStore.subscribe((state) => {
    listener(state.settings);
  });
};

export const updateSettings = async (partial: Partial<Settings>) => {
  const next = {
    ...settingsStore.state.settings,
    ...partial,
  };

  settingsStore.setState((state) => ({
    ...state,
    settings: next,
  }));

  try {
    const store = await getTauriStore();
    await store.set("settings", next);
    await store.save();
  } catch {
    // 설정 저장 실패는 UI 동작을 막지 않음
  }
};

export const initSettingsFromStore = async () => {
  try {
    const store = await getTauriStore();
    const stored = await store.get<Settings>("settings");
    if (stored) {
      settingsStore.setState((state) => ({
        ...state,
        settings: {
          ...DEFAULT_SETTINGS,
          ...stored,
        },
      }));
    }
  } catch {
    // 초기 로드 실패 시 기본값 유지
  }
};

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
