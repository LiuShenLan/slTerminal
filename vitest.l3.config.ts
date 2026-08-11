import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    include: ['test/terminal/**/*.test.ts'],
    exclude: ['node_modules', '.temp', 'e2e-tests'],
    server: {
      deps: {
        // codemirror-json-schema 0.8.1 的 ESM dist 用无扩展名相对导入
        //（dist/index.js → "./features/completion"），Node ESM loader 无法解析——
        // 必须经 Vite 转换（resolver 补全扩展名）。json-schema-library 走 CJS main 入口，无需 inline。
        inline: ['codemirror-json-schema'],
      },
    },
  },
});
