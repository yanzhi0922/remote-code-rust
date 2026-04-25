import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, act, cleanup } from '@testing-library/react';
import {
  useSlashCommandController,
  matchSlashQuery,
  getScrollTopForActiveItem,
  type SlashCommandItem,
} from './useSlashCommandController';

const MOCK_COMMANDS: SlashCommandItem[] = [
  { name: 'help', description: 'Show help', kind: 'builtin' },
  { name: 'clear', description: 'Clear screen', kind: 'builtin' },
  { name: 'model', description: 'Switch model', kind: 'builtin' },
  { name: 'agent', description: 'Run agent', kind: 'template' },
  { name: 'review', description: 'Code review', kind: 'template', selectionBehavior: 'insert' },
];

describe('matchSlashQuery', () => {
  it('matches /command pattern', () => {
    expect(matchSlashQuery('/help')).toBe('help');
    expect(matchSlashQuery('/clear')).toBe('clear');
  });

  it('matches / with no command', () => {
    expect(matchSlashQuery('/')).toBe('');
  });

  it('matches partial command', () => {
    expect(matchSlashQuery('/he')).toBe('he');
  });

  it('returns null for non-slash input', () => {
    expect(matchSlashQuery('hello')).toBeNull();
    expect(matchSlashQuery('')).toBeNull();
    expect(matchSlashQuery('/hello world')).toBeNull();
  });

  it('matches commands with underscores and hyphens', () => {
    expect(matchSlashQuery('/my-command')).toBe('my-command');
    expect(matchSlashQuery('/my_command')).toBe('my_command');
  });
});

describe('getScrollTopForActiveItem', () => {
  it('returns current scrollTop when item is visible', () => {
    expect(
      getScrollTopForActiveItem({
        containerScrollTop: 100,
        containerHeight: 200,
        itemOffsetTop: 150,
        itemOffsetHeight: 30,
      }),
    ).toBe(100);
  });

  it('scrolls up when item is above viewport', () => {
    expect(
      getScrollTopForActiveItem({
        containerScrollTop: 200,
        containerHeight: 200,
        itemOffsetTop: 150,
        itemOffsetHeight: 30,
      }),
    ).toBe(150);
  });

  it('scrolls down when item is below viewport', () => {
    expect(
      getScrollTopForActiveItem({
        containerScrollTop: 0,
        containerHeight: 200,
        itemOffsetTop: 250,
        itemOffsetHeight: 30,
      }),
    ).toBe(80);
  });

  it('returns current scrollTop when containerHeight is 0', () => {
    expect(
      getScrollTopForActiveItem({
        containerScrollTop: 50,
        containerHeight: 0,
        itemOffsetTop: 100,
        itemOffsetHeight: 30,
      }),
    ).toBe(50);
  });
});

describe('useSlashCommandController', () => {
  afterEach(() => {
    cleanup();
  });

  it('returns closed state for non-slash input', () => {
    const { result } = renderHook(() =>
      useSlashCommandController({ input: 'hello', commands: MOCK_COMMANDS }),
    );

    expect(result.current.isOpen).toBe(false);
    expect(result.current.filteredCommands).toEqual([]);
  });

  it('opens when input is /', () => {
    const { result } = renderHook(() =>
      useSlashCommandController({ input: '/', commands: MOCK_COMMANDS }),
    );

    expect(result.current.isOpen).toBe(true);
    expect(result.current.filteredCommands).toHaveLength(MOCK_COMMANDS.length);
  });

  it('filters commands by query', () => {
    const { result } = renderHook(() =>
      useSlashCommandController({ input: '/he', commands: MOCK_COMMANDS }),
    );

    expect(result.current.isOpen).toBe(true);
    expect(result.current.filteredCommands).toHaveLength(1);
    expect(result.current.filteredCommands[0].name).toBe('help');
  });

  it('closes when no commands match', () => {
    const { result } = renderHook(() =>
      useSlashCommandController({ input: '/xyz', commands: MOCK_COMMANDS }),
    );

    expect(result.current.isOpen).toBe(false);
  });

  it('handles ArrowDown navigation', () => {
    const { result } = renderHook(() =>
      useSlashCommandController({ input: '/', commands: MOCK_COMMANDS }),
    );

    expect(result.current.activeIndex).toBe(0);

    act(() => {
      result.current.onKeyDown({ key: 'ArrowDown', preventDefault: vi.fn() } as unknown as React.KeyboardEvent);
    });

    expect(result.current.activeIndex).toBe(1);
  });

  it('handles ArrowUp navigation wrapping', () => {
    const { result } = renderHook(() =>
      useSlashCommandController({ input: '/', commands: MOCK_COMMANDS }),
    );

    expect(result.current.activeIndex).toBe(0);

    act(() => {
      result.current.onKeyDown({ key: 'ArrowUp', preventDefault: vi.fn() } as unknown as React.KeyboardEvent);
    });

    expect(result.current.activeIndex).toBe(MOCK_COMMANDS.length - 1);
  });

  it('handles Escape to dismiss', () => {
    const { result } = renderHook(() =>
      useSlashCommandController({ input: '/', commands: MOCK_COMMANDS }),
    );

    expect(result.current.isOpen).toBe(true);

    act(() => {
      result.current.onKeyDown({ key: 'Escape', preventDefault: vi.fn() } as unknown as React.KeyboardEvent);
    });

    expect(result.current.isOpen).toBe(false);
  });

  it('executes builtin command on Enter', () => {
    const onExecuteBuiltin = vi.fn();
    const { result } = renderHook(() =>
      useSlashCommandController({
        input: '/help',
        commands: MOCK_COMMANDS,
        onExecuteBuiltin,
      }),
    );

    act(() => {
      result.current.onKeyDown({
        key: 'Enter',
        shiftKey: false,
        preventDefault: vi.fn(),
      } as unknown as React.KeyboardEvent);
    });

    expect(onExecuteBuiltin).toHaveBeenCalledWith('help');
  });

  it('executes template command on Enter', () => {
    const onSelectTemplate = vi.fn();
    const { result } = renderHook(() =>
      useSlashCommandController({
        input: '/review',
        commands: MOCK_COMMANDS,
        onSelectTemplate,
      }),
    );

    act(() => {
      result.current.onKeyDown({
        key: 'Enter',
        shiftKey: false,
        preventDefault: vi.fn(),
      } as unknown as React.KeyboardEvent);
    });

    expect(onSelectTemplate).toHaveBeenCalledWith('review');
  });

  it('does not execute on Shift+Enter', () => {
    const onExecuteBuiltin = vi.fn();
    const { result } = renderHook(() =>
      useSlashCommandController({
        input: '/help',
        commands: MOCK_COMMANDS,
        onExecuteBuiltin,
      }),
    );

    act(() => {
      result.current.onKeyDown({
        key: 'Enter',
        shiftKey: true,
        preventDefault: vi.fn(),
      } as unknown as React.KeyboardEvent);
    });

    expect(onExecuteBuiltin).not.toHaveBeenCalled();
  });

  it('returns false from onKeyDown when not open', () => {
    const { result } = renderHook(() =>
      useSlashCommandController({ input: 'hello', commands: MOCK_COMMANDS }),
    );

    const handled = result.current.onKeyDown({
      key: 'ArrowDown',
      preventDefault: vi.fn(),
    } as unknown as React.KeyboardEvent);

    expect(handled).toBe(false);
  });

  it('resets activeIndex when query changes', () => {
    const { result, rerender } = renderHook(
      ({ input }) => useSlashCommandController({ input, commands: MOCK_COMMANDS }),
      { initialProps: { input: '/' } },
    );

    act(() => {
      result.current.onKeyDown({ key: 'ArrowDown', preventDefault: vi.fn() } as unknown as React.KeyboardEvent);
    });

    expect(result.current.activeIndex).toBe(1);

    rerender({ input: '/he' });

    expect(result.current.activeIndex).toBe(0);
  });

  it('executes command via onSelectByIndex', () => {
    const onExecuteBuiltin = vi.fn();
    const { result } = renderHook(() =>
      useSlashCommandController({
        input: '/',
        commands: MOCK_COMMANDS,
        onExecuteBuiltin,
      }),
    );

    act(() => {
      result.current.onSelectByIndex(0);
    });

    expect(onExecuteBuiltin).toHaveBeenCalledWith('help');
  });

  it('returns false from onSelectByIndex for invalid index', () => {
    const { result } = renderHook(() =>
      useSlashCommandController({ input: '/', commands: MOCK_COMMANDS }),
    );

    const executed = result.current.onSelectByIndex(999);
    expect(executed).toBe(false);
  });
});
