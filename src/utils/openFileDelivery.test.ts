import { beforeEach, describe, expect, it, vi } from "vitest"

import {
  deliverOpenFile,
  hasBufferedOpenFile,
  registerOpenFileHandler,
  resetOpenFileDelivery
} from "@/utils/openFileDelivery"

describe("openFileDelivery", () => {
  beforeEach(() => {
    resetOpenFileDelivery()
  })

  it("delivers to the registered handler", () => {
    const handler = vi.fn()
    registerOpenFileHandler(handler)

    deliverOpenFile("D:/a.png")

    expect(handler).toHaveBeenCalledWith("D:/a.png")
  })

  it("buffers the path until a handler registers", () => {
    deliverOpenFile("D:/b.png")
    expect(hasBufferedOpenFile()).toBe(true)

    const handler = vi.fn()
    registerOpenFileHandler(handler)

    expect(handler).toHaveBeenCalledTimes(1)
    expect(handler).toHaveBeenCalledWith("D:/b.png")
    expect(hasBufferedOpenFile()).toBe(false)
  })

  it("keeps only the latest buffered path", () => {
    deliverOpenFile("D:/first.png")
    deliverOpenFile("D:/second.png")

    const handler = vi.fn()
    registerOpenFileHandler(handler)

    expect(handler).toHaveBeenCalledTimes(1)
    expect(handler).toHaveBeenCalledWith("D:/second.png")
  })

  it("stale unregister does not clear a newer handler", () => {
    const first = vi.fn()
    const unregisterFirst = registerOpenFileHandler(first)

    const second = vi.fn()
    registerOpenFileHandler(second)
    unregisterFirst()

    deliverOpenFile("D:/c.png")

    expect(second).toHaveBeenCalledWith("D:/c.png")
    expect(first).not.toHaveBeenCalled()
  })

  it("stops delivering after the active handler unregisters", () => {
    const handler = vi.fn()
    const unregister = registerOpenFileHandler(handler)
    unregister()

    deliverOpenFile("D:/d.png")

    expect(handler).not.toHaveBeenCalled()
  })
})
