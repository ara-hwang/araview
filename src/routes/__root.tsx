import { createRootRoute, Outlet } from "@tanstack/react-router"
import { Toaster } from "sonner"
import { ThemeProvider } from "@/components/theme-provider"
import Header from "@/components/Header"
import { Separator } from "@/components/ui/separator"
import { StatusBar } from "@/components/StatusBar"
import { ExifPanel } from "@/components/ExifPanel"

export const Route = createRootRoute({
  component: RootLayout,
  notFoundComponent: () => <div>Not Found</div>
})

function RootLayout() {
  return (
    <ThemeProvider>
      <div className="flex h-screen w-screen flex-col">
        <Header />
        <Separator />
        <div className="relative flex-1">
          <Outlet />
          <StatusBar />
        </div>
      </div>
      <ExifPanel />
      <Toaster position="bottom-right" richColors closeButton />
    </ThemeProvider>
  )
}
