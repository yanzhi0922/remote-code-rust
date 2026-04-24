import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ConfigurableShortcutHint } from './ConfigurableShortcutHint';

afterEach(() => {
  cleanup();
});

describe('ConfigurableShortcutHint', () => {
  it('renders shortcut hint', () => {
    render(<ConfigurableShortcutHint action="toggle" fallback="ctrl+o" description="展开" />);
    expect(screen.getByTestId('configurable-shortcut-hint')).toBeInTheDocument();
    expect(screen.getByText('展开')).toBeInTheDocument();
    expect(screen.getByText('ctrl+o')).toBeInTheDocument();
  });

  it('wraps in parens by default', () => {
    render(<ConfigurableShortcutHint action="toggle" fallback="ctrl+o" description="展开" />);
    expect(screen.getByTestId('configurable-shortcut-hint-parens')).toBeInTheDocument();
  });

  it('hides parens when disabled', () => {
    render(<ConfigurableShortcutHint action="toggle" fallback="ctrl+o" description="展开" parens={false} />);
    expect(screen.queryByTestId('configurable-shortcut-hint-parens')).not.toBeInTheDocument();
  });
});
