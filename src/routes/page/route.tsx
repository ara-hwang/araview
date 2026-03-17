import { createFileRoute, Outlet, useNavigate } from "@tanstack/react-router"
import { getApp } from "@/store/appStore"
import { Button } from "@/components/ui/button"
import { ArrowLeft } from "lucide-react"

export const Route = createFileRoute("/page")({
  component: RouteComponent
})

function RouteComponent() {
  return (
    <div className="flex flex-col gap-2 p-2">
      <PageHeader />
      <Outlet />
    </div>
  )
}

function PageHeader() {
  const navigate = useNavigate()

  const handleBack = () => {
    void navigate({ to: getApp().imageInfo ? "/image" : "/" })
  }

  return (
    <div className="flex items-center gap-4">
      <Button variant="outline" size="icon" onClick={handleBack} title="Back">
        <ArrowLeft />
      </Button>
      <span className="font-medium">Settings</span>
    </div>
  )
}
