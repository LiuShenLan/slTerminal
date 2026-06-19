import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    include: ['test/terminal/**/*.test.ts'],
    exclude: ['node_modules', '.temp', 'e2e-tests'],
  },
});
