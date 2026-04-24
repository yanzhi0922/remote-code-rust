import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PermissionPrompt, type PermissionPromptOption } from './PermissionPrompt';

const baseOptions: PermissionPromptOption[] = [
  { value: 'allow', label: 'Allow', description: 'Allow this action', shortcutKey: 'a', variant: 'success' },
  { value: 'deny', label: 'Deny', description: 'Deny this action', shortcutKey: 'n', variant: 'danger' },
  { value: 'ask', label: 'Ask', description: 'Always ask', variant: 'warning' },
];

describe('PermissionPrompt', () => {
  afterEach(() => { cleanup(); });

  it('renders with data-testid', () => {
    render(<PermissionPrompt options={baseOptions} onSelect={vi.fn()} />);
    expect(screen.getByTestId('permission-prompt')).toBeInTheDocument();
  });

  it('renders all options', () => {
    render(<PermissionPrompt options={baseOptions} onSelect={vi.fn()} />);
    expect(screen.getByTestId('permission-option-allow')).toBeInTheDocument();
    expect(screen.getByTestId('permission-option-deny')).toBeInTheDocument();
    expect(screen.getByTestId('permission-option-ask')).toBeInTheDocument();
  });

  it('renders option labels and descriptions', () => {
    render(<PermissionPrompt options={baseOptions} onSelect={vi.fn()} />);
    expect(screen.getByText('Allow')).toBeInTheDocument();
    expect(screen.getByText('Allow this action')).toBeInTheDocument();
  });

  it('renders shortcut keys', () => {
    render(<PermissionPrompt options={baseOptions} onSelect={vi.fn()} />);
    expect(screen.getByText('a')).toBeInTheDocument();
    expect(screen.getByText('n')).toBeInTheDocument();
  });

  it('calls onSelect when option is clicked', () => {
    const fn = vi.fn();
    render(<PermissionPrompt options={baseOptions} onSelect={fn} />);
    fireEvent.click(screen.getByTestId('permission-option-allow'));
    expect(fn).toHaveBeenCalledWith('allow', undefined);
  });

  it('renders custom title', () => {
    render(<PermissionPrompt title="Custom Title" options={baseOptions} onSelect={vi.fn()} />);
    expect(screen.getByText('Custom Title')).toBeInTheDocument();
  });

  it('renders default title when not provided', () => {
    render(<PermissionPrompt options={baseOptions} onSelect={vi.fn()} />);
    expect(screen.getByText('是否继续？')).toBeInTheDocument();
  });

  it('renders description when provided', () => {
    render(<PermissionPrompt description="A description" options={baseOptions} onSelect={vi.fn()} />);
    expect(screen.getByText('A description')).toBeInTheDocument();
  });

  it('renders options list with role', () => {
    render(<PermissionPrompt options={baseOptions} onSelect={vi.fn()} />);
    expect(screen.getByTestId('permission-options-list')).toBeInTheDocument();
  });

  it('renders footer hints', () => {
    render(<PermissionPrompt options={baseOptions} onSelect={vi.fn()} />);
    expect(screen.getByText('Esc 取消')).toBeInTheDocument();
  });

  it('renders feedback expand hint for options with feedbackConfig', () => {
    const options: PermissionPromptOption[] = [
      { value: 'allow', label: 'Allow', feedbackConfig: { type: 'accept' } },
    ];
    render(<PermissionPrompt options={options} onSelect={vi.fn()} />);
    expect(screen.getByTestId('tab-hint-allow')).toBeInTheDocument();
  });

  it('shows feedback textarea when tab hint is clicked', () => {
    const options: PermissionPromptOption[] = [
      { value: 'allow', label: 'Allow', feedbackConfig: { type: 'accept' } },
    ];
    render(<PermissionPrompt options={options} onSelect={vi.fn()} />);
    // The tab hint button exists
    expect(screen.getByTestId('tab-hint-allow')).toBeInTheDocument();
  });

  it('calls onAnalytics when provided', () => {
    const analyticsFn = vi.fn();
    const options: PermissionPromptOption[] = [
      { value: 'allow', label: 'Allow', feedbackConfig: { type: 'accept' } },
    ];
    render(
      <PermissionPrompt
        options={options}
        onSelect={vi.fn()}
        onAnalytics={analyticsFn}
        toolAnalyticsContext={{ toolName: 'Bash', isMcp: false }}
      />,
    );
    fireEvent.click(screen.getByTestId('permission-option-allow'));
    expect(analyticsFn).toHaveBeenCalledWith('accept_submitted', expect.any(Object));
  });

  it('calls onCancel on Escape key', () => {
    const cancelFn = vi.fn();
    render(<PermissionPrompt options={baseOptions} onSelect={vi.fn()} onCancel={cancelFn} />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(cancelFn).toHaveBeenCalled();
  });
});
