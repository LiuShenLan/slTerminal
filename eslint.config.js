import js from '@eslint/js';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    rules: {
      'no-restricted-imports': ['error', {
        paths: [{
          name: '@tauri-apps/api/core',
          importNames: ['invoke'],
          message: 'invoke 只允许在 src/ipc/ 目录内使用。请通过 src/ipc/ 暴露的领域函数间接调用。',
        }],
        patterns: [{
          group: ['@tauri-apps/plugin-*'],
          message: '@tauri-apps/plugin-* 包只允许在 src/ipc/ 目录内使用。请通过 src/ipc/ 暴露的领域函数间接调用。',
        }],
      }],
    },
  },
  // 对 src/ipc/ 目录放行 no-restricted-imports
  {
    files: ['src/ipc/**/*.ts', 'src/ipc/**/*.tsx'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },
);
