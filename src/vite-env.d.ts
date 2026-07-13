/// <reference types="vite/client" />

// E2E 构建开关：由 `cross-env VITE_E2E=1`（build:e2e）设置，暴露到 import.meta.env。
// 见 src/lib/e2eEnabled.ts 与 e2e-tests/CLAUDE.md。
interface ImportMetaEnv {
  readonly VITE_E2E?: string;
}
