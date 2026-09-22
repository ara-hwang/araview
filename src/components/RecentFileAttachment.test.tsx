import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) =>
      options ? `${key}:${Object.values(options).join("/")}` : key,
    i18n: { language: "en" }
  })
}))

import { extOf, parentDir, RecentFileAttachment } from "@/components/RecentFileAttachment"

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

const archiveInfo = {
  file_path: "C:/comics/m.cbz",
  mime_type: "application/vnd.comicbook+zip",
  file_name: "m.cbz",
  file_size: 1024,
  width: null,
  height: null
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
      container.querySelector('[data-slot="attachment-media"]')?.getAttribute("data-variant")
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

    fireEvent.click(screen.getByRole("button", { name: "home.card.remove, photo.png" }))
    expect(onRemove).toHaveBeenCalledWith("/pics/photo.png")

    fireEvent.click(screen.getByRole("button", { name: "home.list.open:photo.png" }))
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

  it("아카이브 카드에 읽기 진도를 함께 표시한다", () => {
    render(
      <RecentFileAttachment
        path={archiveInfo.file_path}
        info={archiveInfo}
        status="done"
        progress={{ entry: "010.jpg", index: 9, total: 340 }}
        onOpen={() => {}}
        onRemove={() => {}}
      />
    )

    expect(
      screen.getByText((content) => content.includes("home.card.progress:10/340"))
    ).not.toBeNull()
  })

  it("진도 기록이 없거나 이미지 카드면 진도 문구를 넣지 않는다", () => {
    render(
      <RecentFileAttachment
        path={archiveInfo.file_path}
        info={archiveInfo}
        status="done"
        onOpen={() => {}}
        onRemove={() => {}}
      />
    )
    expect(screen.queryByText((content) => content.includes("home.card.progress"))).toBeNull()

    cleanup()

    render(
      <RecentFileAttachment
        path="/pics/photo.png"
        info={imgInfo}
        status="done"
        progress={{ entry: "010.jpg", index: 9, total: 340 }}
        onOpen={() => {}}
        onRemove={() => {}}
      />
    )
    expect(screen.getByText("PNG · 2.0 KB · 800x600")).not.toBeNull()
    expect(screen.queryByText((content) => content.includes("home.card.progress"))).toBeNull()
  })
})
