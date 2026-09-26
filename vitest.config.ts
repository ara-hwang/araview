import path from "node:path"

import { defineConfig } from "vitest/config"

export default defineConfig({
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    // 병렬 실행 중 CPU를 다른 작업이 점유하면 jsdom 테스트가 수 초씩 늘어날 수 있다.
    // 기본 5초는 그 상황에서 조용히 실패하므로 여유를 준다(asyncUtilTimeout은
    // vitest.setup.ts의 waitFor 기본값).
    testTimeout: 15_000,
    setupFiles: ["./vitest.setup.ts"],
    coverage: {
      provider: "v8",
      reporter: ["text", "html"],
      reportsDirectory: "coverage",
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.test.{ts,tsx}",
        "src/components/ui/**",
        "src/routeTree.gen.ts",
        "src/types/**",
        "src/i18n/locales/**"
      ]
    }
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src")
    }
  }
})
