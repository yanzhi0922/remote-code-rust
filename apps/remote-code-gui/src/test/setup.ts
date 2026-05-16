import '@testing-library/jest-dom/vitest';

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
