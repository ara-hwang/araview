import type { DirectoryImages, FileAvailability } from "@/types"

export function availabilityAt(dirImages: DirectoryImages, index: number): FileAvailability {
  const list = dirImages.availability
  if (!list?.length || index < 0 || index >= list.length) {
    return "unknown"
  }
  return list[index]
}

export function isCloudOnlyPath(dirImages: DirectoryImages, path: string): boolean {
  const index = dirImages.images.indexOf(path)
  if (index < 0) {
    return false
  }
  return availabilityAt(dirImages, index) === "cloud_only"
}

export function cloudOnlyPathSet(dirImages: DirectoryImages): ReadonlySet<string> {
  const out = new Set<string>()
  const list = dirImages.availability
  if (!list?.length) {
    return out
  }
  dirImages.images.forEach((path, index) => {
    if (list[index] === "cloud_only") {
      out.add(path)
    }
  })
  return out
}
