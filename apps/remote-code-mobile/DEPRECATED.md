# DEPRECATED

This Capacitor mobile app is **deprecated** and will be removed in a future release.

Mobile support has been unified into the main Tauri GUI app (`apps/remote-code-gui/`).

The Tauri v2 mobile build targets (iOS + Android) now provide all mobile-native capabilities:

- Biometric authentication
- Push notifications
- Network monitoring
- Deep link handling (QR pairing)
- Haptic feedback
- Secure storage
- File download & share

## Migration

To build for mobile, use the Tauri CLI from the GUI app directory:

```bash
cd apps/remote-code-gui

# iOS (macOS only)
npm run tauri ios init    # first time only
npm run tauri ios dev     # development
npm run tauri ios build   # production

# Android
npm run tauri android init  # first time only
npm run tauri android dev   # development
npm run tauri android build # production
```

Or from the repo root:

```bash
make dev-ios
make dev-android
make build-ios
make build-android
```

## Why Tauri instead of Capacitor?

1. **Code reuse**: Same Rust backend + React frontend covers desktop + mobile
2. **Native Rust on device**: rc-* crates run directly on the phone, no WebView → HTTP API bridge
3. **Single dependency tree**: One set of plugins (Tauri), not two (Capacitor + Tauri)
4. **Smaller binary size**: Tauri uses the system WebView instead of bundling Chromium
