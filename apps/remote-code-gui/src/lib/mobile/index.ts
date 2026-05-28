export { isMobileSync, isTouchDevice } from './platform';

export {
  performBiometricCheck,
  getBiometricEnabled,
  setBiometricEnabled,
} from './biometric';

export {
  hapticSuccess,
  hapticWarning,
  hapticError,
} from './haptics';

export {
  initNetworkMonitoring,
  getNetworkStatus,
  onNetworkChange,
  describeConnectionType,
} from './network';
