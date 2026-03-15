import type { Settings } from "../types";

export function getCacheLimit(mode: Settings["cacheMode"]): number {
  switch (mode) {
    case "off":
      return 1;
    case "extended":
      return 64;
    case "nearby":
    default:
      return 24;
  }
}

export function getPrefetchDistance(mode: Settings["cacheMode"]): number {
  switch (mode) {
    case "off":
      return 0;
    case "extended":
      return 3;
    case "nearby":
    default:
      return 1;
  }
}
