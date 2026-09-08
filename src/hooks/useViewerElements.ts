import { useEffect, useRef } from "react"
import { useAppStore } from "@/store/appStore"

export function useViewerElements() {
  const containerRef = useRef<HTMLDivElement>(null)
  const imageRef = useRef<HTMLImageElement>(null)

  useEffect(() => {
    useAppStore.setState({ containerElement: containerRef.current })
    useAppStore.setState({ imageElement: imageRef.current })
  }, [])

  useEffect(() => {
    const updateViewport = () => {
      useAppStore.setState({
        viewportSize: { width: window.innerWidth, height: window.innerHeight }
      })
    }

    updateViewport()
    window.addEventListener("resize", updateViewport)
    return () => {
      window.removeEventListener("resize", updateViewport)
    }
  }, [])

  useEffect(() => {
    const containerElement = containerRef.current
    if (!containerElement) return

    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (entry.target === containerElement) {
          useAppStore.setState({
            containerSize: {
              width: entry.contentRect.width,
              height: entry.contentRect.height
            }
          })
        }
      }
    })

    resizeObserver.observe(containerElement)

    useAppStore.setState({
      containerSize: {
        width: containerElement.clientWidth,
        height: containerElement.clientHeight
      }
    })

    return () => {
      resizeObserver.disconnect()
    }
  }, [])

  return { containerRef, imageRef }
}
