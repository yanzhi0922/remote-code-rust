/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_REMOTE_CONTROL_PLANE_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

declare const __APP_BUILD_ID__: string;
