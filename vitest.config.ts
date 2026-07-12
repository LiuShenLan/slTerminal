import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['src/__tests__/**/*.test.ts', 'src/__tests__/**/*.test.tsx'],
    exclude: ['node_modules', 'datalearncodeterax-ai-temp', '.temp', 'e2e-tests'],
    setupFiles: ['src/__tests__/setup.ts'],
  },
});
