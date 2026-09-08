import { describe, expect, it } from "vitest"
import { hasBlockedAssociation } from "./useFileAssociations"
import type { FileAssociation } from "@/types"

function association(
  partial: Partial<FileAssociation> & Pick<FileAssociation, "extension">
): FileAssociation {
  return {
    associated: false,
    current_prog_id: null,
    needs_os_confirmation: false,
    ...partial
  }
}

describe("hasBlockedAssociation", () => {
  it("is true when Windows UserChoice blocks an association", () => {
    expect(
      hasBlockedAssociation([
        association({
          extension: "png",
          associated: false,
          needs_os_confirmation: true,
          current_prog_id: "AppXPhotos"
        })
      ])
    ).toBe(true)
  })

  it("is false when every extension is associated or unrestricted", () => {
    expect(
      hasBlockedAssociation([
        association({ extension: "png", associated: true }),
        association({
          extension: "jpg",
          associated: false,
          needs_os_confirmation: false
        })
      ])
    ).toBe(false)
  })
})
