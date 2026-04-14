import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

/**
 * Vite plugin that rewrites `../lib/*` imports from the shared remote/
 * directory to use our mobile-specific versions.
 *
 * - `../lib/runtime` → mobile runtime (Capacitor Preferences storage)
 * - `../lib/utils`   → local copy (identical to web version)
 */
function mobileLibRewrite(): Plugin {
  const libDir = path.resolve(__dirname, 'src/lib');
  const sharedRemoteMarker = path.normalize('remote-code-gui/src/remote');

  return {
    name: 'mobile-lib-rewrite',
    enforce: 'pre',
    resolveId(source, importer) {
      if (!importer || !path.normalize(importer).includes(sharedRemoteMarker)) {
        return null;
      }

      // Rewrite ../lib/runtime → our mobile runtime
      // Rewrite ../lib/utils   → our local copy
      if (source === '../lib/runtime') {
        return path.resolve(libDir, 'runtime.ts');
      }
      if (source === '../lib/utils') {
        return path.resolve(libDir, 'utils.ts');
      }

      return null;
    },
  };
}

export default defineConfig({
  plugins: [react(), mobileLibRewrite()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
      // Shared remote code from the GUI project
      '@remote': path.resolve(__dirname, '../remote-code-gui/src/remote'),
    },
  },
  build: {
    chunkSizeWarningLimit: 700,
    rollupOptions: {
      // External modules that are loaded at runtime on native platforms
      external: ['@capawesome-team/capacitor-biometrics'],
      output: {
        manualChunks: {
          react: ['react', 'react-dom'],
          ui: ['lucide-react', 'clsx', 'tailwind-merge'],
          'markdown-rendering': [
            'react-markdown',
            'remark-gfm',
            'remark-math',
            'rehype-katex',
            'rehype-highlight',
          ],
          capacitor: [
            '@capacitor/core',
            '@capacitor/preferences',
            '@capacitor/app',
            '@capacitor/network',
          ],
        },
      },
    },
  },
  server: {
    port: 1421,
    strictPort: true,
  },
});
