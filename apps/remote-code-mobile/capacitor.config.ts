import type { CapacitorConfig } from '@capacitor/cli';

const config: CapacitorConfig = {
  appId: 'com.remotecode.app',
  appName: 'Remote Code',
  webDir: 'dist',
  server: {
    // In development, you can set this to your dev server URL:
    // url: 'http://192.168.x.x:1421',
    // cleartext: true,
    androidScheme: 'https',
  },
  plugins: {
    StatusBar: {
      style: 'DARK',
      backgroundColor: '#17181a',
    },
    Preferences: {
      group: 'RemoteCode',
    },
    SplashScreen: {
      launchShowDuration: 2000,
      launchAutoHide: true,
      backgroundColor: '#f4efe4',
      showSpinner: true,
      spinnerColor: '#64748b',
      androidScaleType: 'CENTER_CROP',
    },
    PushNotifications: {
      presentationOptions: ['badge', 'sound', 'alert'],
    },
  },
  ios: {
    contentInset: 'never',
    preferredContentMode: 'mobile',
  },
  android: {
    minWebViewVersion: 60,
  },
};

export default config;
