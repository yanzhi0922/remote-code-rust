/**
 * Type declarations for the optional @capawesome-team/capacitor-biometrics plugin.
 *
 * This plugin is loaded via dynamic import at runtime on native platforms.
 * It may not be installed during development, so we provide minimal type
 * declarations here to satisfy TypeScript.
 */
declare module '@capawesome-team/capacitor-biometrics' {
  export interface AvailabilityResult {
    available: boolean;
  }

  export interface AuthenticateOptions {
    reason: string;
    iosFallbackTitle?: string;
    androidTitle?: string;
    androidSubtitle?: string;
    androidConfirmationRequired?: boolean;
  }

  export function checkAvailability(): Promise<AvailabilityResult>;
  export function authenticate(options: AuthenticateOptions): Promise<void>;
}
