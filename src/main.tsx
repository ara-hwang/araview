import { createRouter, RouterProvider } from "@tanstack/react-router"
import React from "react"
import ReactDOM from "react-dom/client"

import { applyAlwaysOnTopFromSettings } from "@/hooks/useAlwaysOnTop"
import { initI18n, detectSystemLanguage } from "@/i18n"
import { useArchiveProgressStore } from "@/store/archiveProgressStore"
import { usePaletteMruStore } from "@/store/paletteMruStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { initSettingsFromStore } from "@/store/settingsStore"

import { routeTree } from "./routeTree.gen"

import "./App.css"

async function bootstrap() {
  await initI18n(detectSystemLanguage())
  await initSettingsFromStore()
  await applyAlwaysOnTopFromSettings()
  await Promise.all([
    useRecentFilesStore.getState().init(),
    usePaletteMruStore.getState().init(),
    useArchiveProgressStore.getState().init()
  ])
}

void bootstrap()

const router = createRouter({ routeTree })

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router
  }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RouterProvider router={router} />
  </React.StrictMode>
)
