import React from "react"
import ReactDOM from "react-dom/client"
import { createRouter, RouterProvider } from "@tanstack/react-router"
import { routeTree } from "./routeTree.gen"
import { initI18n, detectSystemLanguage } from "@/i18n"
import { initSettingsFromStore } from "@/store/settingsStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { useFavoritesStore } from "@/store/favoritesStore"
import "./App.css"

async function bootstrap() {
  await initI18n(detectSystemLanguage())
  await initSettingsFromStore()
  await Promise.all([
    useRecentFilesStore.getState().init(),
    useFavoritesStore.getState().init()
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
