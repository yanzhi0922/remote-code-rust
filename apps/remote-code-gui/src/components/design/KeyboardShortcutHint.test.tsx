import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { KeyboardShortcutHint } from './KeyboardShortcutHint';

describe('KeyboardShortcutHint', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<KeyboardShortcutHint keys={['Ctrl', 'K']} description="打开命令面板" />);
    expect(screen.getByTestId('keyboard-shortcut-hint')).toBeInTheDocument();
  });

  it('renders key badges', () => {
    render(<KeyboardShortcutHint keys={['Ctrl', 'K']} description="打开命令面板" />);
    expect(screen.getByTestId('shortcut-key-Ctrl')).toBeInTheDocument();
    expect(screen.getByTestId('shortcut-key-K')).toBeInTheDocument();
  });

  it('renders description text', () => {
    render(<KeyboardShortcutHint keys={['Ctrl', 'S']} description="保存文件" />);
    expect(screen.getByTestId('shortcut-description')).toHaveTextContent('保存文件');
  });

  it('renders single key shortcut', () => {
    render(<KeyboardShortcutHint keys={['Esc']} description="取消" />);
    expect(screen.getByTestId('shortcut-key-Esc')).toBeInTheDocument();
  });

  it('renders three-key combination', () => {
    render(<KeyboardShortcutHint keys={['Ctrl', 'Shift', 'P']} description="命令面板" />);
    expect(screen.getByTestId('shortcut-key-Ctrl')).toBeInTheDocument();
    expect(screen.getByTestId('shortcut-key-Shift')).toBeInTheDocument();
    expect(screen.getByTestId('shortcut-key-P')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    render(<KeyboardShortcutHint keys={['Ctrl', 'K']} description="test" className="extra" />);
    expect(screen.getByTestId('keyboard-shortcut-hint').className).toContain('extra');
  });
});
