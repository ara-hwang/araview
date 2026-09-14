import { afterEach, describe, expect, it, vi } from "vitest"
import { cleanup, fireEvent, render, screen } from "@testing-library/react"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, string>) =>
      options?.name ? `${key}:${options.name}` : key,
    i18n: { language: "en" }
  })
}))

import {
  extOf,
  parentDir,
  RecentFileAttachment
} from "@/components/RecentFileAttachment"

afterEach(() => {
  cleanup()
})

const imgInfo = {
  file_path: "/pics/photo.png",
  mime_type: "image/png",
  file_name: "photo.png",
  file_size: 2048,
  width: 800,
  height: 600
}

describe("recent path helpers", () => {
  it("확장자를 대문자로 반환한다", () => {
    expect(extOf("C:\\pics\\photo.JPG")).toBe("JPG")
    expect(extOf("/pics/noext")).toBe("")
  })

  it("상위 폴더를 반환한다", () => {
    expect(parentDir("C:\\pics\\photo.png")).toBe("C:\\pics")
    expect(parentDir("/pics/photo.png")).toBe("/pics")
  })
})

describe("RecentFileAttachment", () => {
  it("파일명과 형식/크기/치수, 폴더를 표시한다", () => {
    const { container } = render(
      <RecentFileAttachment
        path="/pics/photo.png"
        info={imgInfo}
        src="asset:///pics/photo.png"
        status="done"
        onOpen={() => {}}
        onRemove={() => {}}
      />
    )

    expect(screen.getByText("photo.png")).not.toBeNull()
    expect(screen.getByText("PNG · 2.0 KB · 800x600")).not.toBeNull()
    expect(screen.getByText("/pics")).not.toBeNull()
    expect(container.querySelector("img")).not.toBeNull()
    expect(
      container
        .querySelector('[data-slot="attachment-media"]')
        ?.getAttribute("data-variant")
    ).toBe("image")
  })

  it("열기/제거 동작을 전달한다", () => {
    const onOpen = vi.fn()
    const onRemove = vi.fn()
    render(
      <RecentFileAttachment
        path="/pics/photo.png"
        info={imgInfo}
        src="asset:///pics/photo.png"
        status="done"
        onOpen={onOpen}
        onRemove={onRemove}
      />
    )

    fireEvent.click(
      screen.getByRole("button", { name: "home.card.remove, photo.png" })
    )
    expect(onRemove).toHaveBeenCalledWith("/pics/photo.png")

    fireEvent.click(
      screen.getByRole("button", { name: "home.list.open:photo.png" })
    )
    expect(onOpen).toHaveBeenCalledWith("/pics/photo.png")
  })

  it("실패 항목은 error 상태로 안내한다", () => {
    const { container } = render(
      <RecentFileAttachment
        path="/pics/gone.png"
        status="error"
        onOpen={() => {}}
        onRemove={() => {}}
      />
    )

    expect(screen.getByText("home.list.unavailable")).not.toBeNull()
    expect(container.querySelector('[data-state="error"]')).not.toBeNull()
  })
})
