import { createRootRoute, Outlet } from "@tanstack/react-router";
import { ThemeProvider } from "@/components/theme-provider";
import Header from "@/components/Header";
import { Separator } from "@/components/ui/separator";
import { StatusBar } from "@/components/StatusBar";

export const Route = createRootRoute({
  component: RootLayout,
  notFoundComponent: () => <div>Not Found</div>,
});

function RootLayout() {
  return (
    <ThemeProvider>
      <div className="flex flex-col h-screen w-screen">
        <Header />
        <Separator />
        <div className="flex-1 relative">
          <Outlet />
          <StatusBar />
        </div>
      </div>
    </ThemeProvider>
  );
}
