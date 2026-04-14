import { defineConfig, mergeConfig } from 'vitest/config';
import viteConfig from './vite.config';

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      environmentOptions: {
        jsdom: {
          url: 'https://remote-code.test/?mode=remote',
        },
      },
      globals: true,
      setupFiles: ['./src/test/setup.ts'],
    },
  }),
);
