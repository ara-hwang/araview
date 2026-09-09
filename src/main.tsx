import React from "react"
import ReactDOM from "react-dom/client"
import { createRouter, RouterProvider } from "@tanstack/react-router"
import { routeTree } from "./routeTree.gen"
import { initSettingsFromStore } from "@/store/settingsStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { useFavoritesStore } from "@/store/favoritesStore"
import "./App.css"

void initSettingsFromStore()
void useRecentFilesStore.getState().init()
void useFavoritesStore.getState().init()

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
