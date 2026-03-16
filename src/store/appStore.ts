import { useEffect, useState } from "react";
import { createStore } from "@tanstack/store";
import { DirectoryImages, ImageInfo } from "@/types";

type App = {
  theme: "system" | "light" | "dark";
  zoom: number;
  dirImages: DirectoryImages;
  imageInfo: ImageInfo | null;
  error: string | null;
  loading: boolean;
  position: { x: number; y: number };
  isDragging: boolean;
};

const appStore = createStore<{ app: App }>({
  app: {
    theme: "system",
    zoom: 1,
    imageInfo: null,
    dirImages: {
      images: [],
      current_index: 0,
    },
    error: null,
    loading: false,
    position: { x: 0, y: 0 },
    isDragging: false,
  },
});

export const getApp = () => appStore.state.app;

export const subscribeToApp = (listener: (app: App) => void) => {
  return appStore.subscribe((state) => {
    listener(state.app);
  });
};

export const updateApp = async (partial: Partial<App>) => {
  const next = {
    ...appStore.state.app,
    ...partial,
  };

  appStore.setState((state) => ({
    ...state,
    app: next,
  }));
};

export const useAppStore = () => {
  const [app, setApp] = useState<App>(() => getApp());

  useEffect(() => {
    const subscription = subscribeToApp((next) => {
      setApp(next);
    });

    return () => {
      subscription.unsubscribe();
    };
  }, []);

  return app;
};
