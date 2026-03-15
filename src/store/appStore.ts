import { createStore } from "@tanstack/store";
import { Store as TauriStore } from "@tauri-apps/plugin-store";
import type { Settings } from "../types";

const DEFAULT_SETTINGS: Settings = {
  background: "checkered",
  loopNavigation: false,
  cacheMode: "nearby",
};

const appStore = createStore<{ settings: Settings }>({
  settings: DEFAULT_SETTINGS,
});

let tauriStorePromise: Promise<TauriStore> | null = null;

const getTauriStore = (): Promise<TauriStore> => {
  if (!tauriStorePromise) {
    tauriStorePromise = TauriStore.load("settings.json");
  }
  return tauriStorePromise;
};

export const getSettings = () => appStore.state.settings;

export const subscribeToSettings = (listener: (settings: Settings) => void) => {
  return appStore.subscribe((state) => {
    listener(state.settings);
  });
};

export const updateSettings = async (partial: Partial<Settings>) => {
  const next = {
    ...appStore.state.settings,
    ...partial,
  };

  appStore.setState((state) => ({
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
    console.log(stored);
    if (stored) {
      appStore.setState((state) => ({
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
