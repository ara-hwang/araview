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
    const imageElement = imageRef.current
    if (!containerElement) return

    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const target = entry.target
        if (target === containerElement) {
          useAppStore.setState({
            containerSize: {
              width: entry.contentRect.width,
              height: entry.contentRect.height
            }
          })
        } else if (target === imageElement) {
          useAppStore.setState({
            imageSize: {
              width: entry.contentRect.width,
              height: entry.contentRect.height
            }
          })
        }
      }
    })

    resizeObserver.observe(containerElement)
    if (imageElement) resizeObserver.observe(imageElement)

    useAppStore.setState({
      containerSize: {
        width: containerElement.clientWidth,
        height: containerElement.clientHeight
      }
    })
    if (imageElement) {
      useAppStore.setState({
        imageSize: {
          width: imageElement.clientWidth,
          height: imageElement.clientHeight
        }
      })
    }

    return () => {
      resizeObserver.disconnect()
    }
  }, [])

  return { containerRef, imageRef }
}
