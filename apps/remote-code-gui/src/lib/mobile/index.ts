export { getPlatform, isMobile, isMobileSync, isTouchDevice } from './platform';
export type { Platform } from './platform';

export {
  secureStoreGet,
  secureStoreSet,
  secureStoreRemove,
} from './secureStorage';

export {
  checkBiometricAvailability,
  authenticateWithBiometrics,
  performBiometricCheck,
  getBiometricEnabled,
  setBiometricEnabled,
} from './biometric';

export {
  hapticImpact,
  hapticNotification,
  hapticSelection,
  hapticLight,
  hapticMedium,
  hapticHeavy,
  hapticSuccess,
  hapticWarning,
  hapticError,
} from './haptics';

export {
  initNetworkMonitoring,
  getNetworkStatus,
  isOnline,
  onNetworkChange,
  describeConnectionType,
} from './network';

export type { NetworkStatus } from './network';

export {
  initDeepLinks,
  parseDeepLink,
  parsePairingUrl,
  buildPairingUrl,
} from './deepLink';

export type { DeepLinkPairing } from './deepLink';

export {
  downloadArtifact,
  shareFile,
  isFileDownloaded,
  readDownloadedTextFile,
} from './fileDownload';

export {
  initAppLifecycle,
  isAppActive,
  isNetworkConnected,
} from './appLifecycle';

export {
  initPushNotifications,
  requestPushPermission,
  registerPushTokenWithControlPlane,
  getStoredPushToken,
  clearPushToken,
} from './pushNotifications';
