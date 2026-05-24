/**
 * Shared utilities for remote session UI components.
 */

/**
 * Extract a human-readable error message from an unknown thrown value.
 */
export function extractErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  return typeof error === 'string' ? error : String(error);
}
