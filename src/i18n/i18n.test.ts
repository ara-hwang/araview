import { describe, expect, it } from "vitest"
import en from "./locales/en.json"
import ko from "./locales/ko.json"

type JsonMap = Record<string, unknown>

function flatten(obj: JsonMap, prefix = ""): Map<string, string> {
  const out = new Map<string, string>()
  for (const [key, value] of Object.entries(obj)) {
    const path = prefix ? `${prefix}.${key}` : key
    if (value !== null && typeof value === "object" && !Array.isArray(value)) {
      for (const [k, v] of flatten(value as JsonMap, path)) out.set(k, v)
    } else if (typeof value === "string") {
      out.set(path, value)
    }
  }
  return out
}

function placeholders(text: string): string[] {
  return [...text.matchAll(/\{\{(\w+)\}\}/g)].map((m) => m[1]).sort()
}

describe("i18n locales", () => {
  it("has the same keys in ko and en", () => {
    const koKeys = [...flatten(ko as JsonMap).keys()].sort()
    const enKeys = [...flatten(en as JsonMap).keys()].sort()
    const missingInEn = koKeys.filter((k) => !enKeys.includes(k))
    const missingInKo = enKeys.filter((k) => !koKeys.includes(k))
    expect({ missingInEn, missingInKo }).toEqual({
      missingInEn: [],
      missingInKo: []
    })
  })

  it("uses the same interpolation placeholders", () => {
    const koFlat = flatten(ko as JsonMap)
    const enFlat = flatten(en as JsonMap)
    const mismatched: string[] = []
    for (const [key, koText] of koFlat) {
      const enText = enFlat.get(key)
      if (!enText) continue
      const a = placeholders(koText)
      const b = placeholders(enText)
      if (a.join(",") !== b.join(",")) mismatched.push(key)
    }
    expect(mismatched).toEqual([])
  })

  it("has no empty translations", () => {
    for (const [name, flat] of [
      ["ko", flatten(ko as JsonMap)],
      ["en", flatten(en as JsonMap)]
    ] as const) {
      for (const [key, value] of flat) {
        expect(value.trim().length, `${name}:${key}`).toBeGreaterThan(0)
      }
    }
  })
})
