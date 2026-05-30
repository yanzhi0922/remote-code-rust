import '@testing-library/jest-dom/vitest';
import zh from '../i18n/locales/zh.json';

// Mock ResizeObserver for jsdom
class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver = ResizeObserverMock;

// Mock scrollIntoView for jsdom
Element.prototype.scrollIntoView = function scrollIntoView() {};

function createStorageMock() {
  let store: Record<string, string> = {};
  return {
    getItem: (key: string) => store[key] ?? null,
    setItem: (key: string, value: string) => { store[key] = value; },
    removeItem: (key: string) => { delete store[key]; },
    clear: () => { store = {}; },
    get length() { return Object.keys(store).length; },
    key: (index: number) => Object.keys(store)[index] ?? null,
  };
}

Object.defineProperty(globalThis, 'localStorage', { value: createStorageMock() });
Object.defineProperty(globalThis, 'sessionStorage', { value: createStorageMock() });

function resolveTranslation(key: string, params?: Record<string, unknown>): string {
  const parts = key.split('.');
  let current: unknown = zh;
  for (const part of parts) {
    if (current && typeof current === 'object' && part in current) {
      current = (current as Record<string, unknown>)[part];
    } else {
      return key;
    }
  }
  if (typeof current !== 'string') return key;
  let result = current;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      result = result.replace(new RegExp(`\\{\\{${k}\\}\\}`, 'g'), String(v));
    }
  }
  return result;
}

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: resolveTranslation,
    i18n: { language: 'zh', changeLanguage: vi.fn(), t: resolveTranslation },
  }),
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  Trans: ({ i18nKey }: { i18nKey: string }) => resolveTranslation(i18nKey),
}));

vi.mock('../i18n', () => ({
  default: { t: resolveTranslation, language: 'zh', changeLanguage: vi.fn() },
}));
