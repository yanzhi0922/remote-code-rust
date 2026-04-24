import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { HighlightedCode } from './HighlightedCode';
import { Markdown } from './Markdown';
import { ModelPicker } from './ModelPicker';
import { Stats } from './Stats';
import { StatusNotices } from './StatusNotices';
import { ThinkingToggle } from './ThinkingToggle';
import { TokenWarning, getWarningLevel } from './TokenWarning';
import { FastIcon } from './FastIcon';
import { FilePathLink } from './FilePathLink';

afterEach(() => {
  cleanup();
});

describe('HighlightedCode', () => {
  it('renders code with line numbers', () => {
    const { container } = render(<HighlightedCode code="line1\nline2" filePath="test.ts" />);
    expect(screen.getByTestId('highlighted-code')).toBeInTheDocument();
    expect(screen.getByText('test.ts')).toBeInTheDocument();
    const codeLines = container.querySelectorAll<HTMLDivElement>('[data-testid="highlighted-code"] pre code > div');
    expect(codeLines.length).toBeGreaterThanOrEqual(1);
    const pre = container.querySelector('pre');
    expect(pre?.textContent).toContain('line1');
    expect(pre?.textContent).toContain('line2');
  });

  it('shows language badge', () => {
    render(<HighlightedCode code="x" filePath="test.py" language="python" />);
    expect(screen.getByText('python')).toBeInTheDocument();
  });

  it('applies dim styling', () => {
    render(<HighlightedCode code="x" filePath="test.ts" dim={true} />);
    expect(screen.getByTestId('highlighted-code').classList.contains('opacity-60')).toBe(true);
  });

  it('shows line numbers', () => {
    const { container } = render(<HighlightedCode code="a\nb\nc" filePath="test.ts" />);
    const lineNums = container.querySelectorAll<HTMLSpanElement>('[data-testid="highlighted-code"] .select-none');
    expect(lineNums.length).toBeGreaterThanOrEqual(1);
    expect(lineNums[0].textContent).toBe('1');
  });

  it('handles empty code', () => {
    render(<HighlightedCode code="" filePath="empty.ts" />);
    expect(screen.getByTestId('highlighted-code')).toBeInTheDocument();
  });
});

describe('Markdown', () => {
  it('renders plain text', () => {
    render(<Markdown>Hello world</Markdown>);
    expect(screen.getByTestId('markdown')).toBeInTheDocument();
    expect(screen.getByText('Hello world')).toBeInTheDocument();
  });

  it('renders headers', () => {
    render(<Markdown>{"# Title\n## Subtitle\n### Section"}</Markdown>);
    expect(screen.getByText('Title')).toBeInTheDocument();
    expect(screen.getByText('Subtitle')).toBeInTheDocument();
    expect(screen.getByText('Section')).toBeInTheDocument();
  });

  it('renders code blocks', () => {
    render(<Markdown>{"```js\nconst x = 1;\n```"}</Markdown>);
    expect(screen.getByTestId('markdown-code-block')).toBeInTheDocument();
    expect(screen.getByText('const x = 1;')).toBeInTheDocument();
  });

  it('renders inline code', () => {
    render(<Markdown>{"Use `npm install` to install"}</Markdown>);
    expect(screen.getByText('npm install')).toBeInTheDocument();
  });

  it('renders bold text', () => {
    render(<Markdown>{"This is **bold** text"}</Markdown>);
    expect(screen.getByText('bold')).toBeInTheDocument();
  });

  it('applies dimColor', () => {
    render(<Markdown dimColor>text</Markdown>);
    expect(screen.getByTestId('markdown').classList.contains('opacity-60')).toBe(true);
  });

  it('renders list items', () => {
    render(<Markdown>{"- item1\n- item2"}</Markdown>);
    expect(screen.getByText('item1')).toBeInTheDocument();
    expect(screen.getByText('item2')).toBeInTheDocument();
  });
});

describe('ModelPicker', () => {
  const models = [
    { value: 'gpt-4', label: 'GPT-4', description: 'Most capable' },
    { value: 'gpt-3.5', label: 'GPT-3.5', description: 'Fast' },
  ];

  it('renders model options', () => {
    render(<ModelPicker models={models} currentModel="gpt-4" onSelect={vi.fn()} />);
    expect(screen.getByTestId('model-picker')).toBeInTheDocument();
    expect(screen.getByText('GPT-4')).toBeInTheDocument();
    expect(screen.getByText('GPT-3.5')).toBeInTheDocument();
  });

  it('shows check on current model', () => {
    render(<ModelPicker models={models} currentModel="gpt-4" onSelect={vi.fn()} />);
    expect(screen.getByTestId('model-option-gpt-4')).toBeInTheDocument();
  });

  it('calls onSelect when model clicked', () => {
    const onSelect = vi.fn();
    render(<ModelPicker models={models} currentModel="gpt-4" onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('model-option-gpt-3.5'));
    expect(onSelect).toHaveBeenCalledWith('gpt-3.5');
  });

  it('shows cancel button when onCancel provided', () => {
    render(<ModelPicker models={models} currentModel={null} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByTestId('model-picker-cancel')).toBeInTheDocument();
  });

  it('shows model descriptions', () => {
    render(<ModelPicker models={models} currentModel={null} onSelect={vi.fn()} />);
    expect(screen.getByText('Most capable')).toBeInTheDocument();
  });
});

describe('Stats', () => {
  const mockStats = {
    totalSessions: 42,
    totalTokens: 1_500_000,
    totalCost: 12.5,
    modelsUsed: { 'gpt-4': 30, 'gpt-3.5': 12 },
  };

  it('renders stats data', () => {
    render(<Stats stats={mockStats} />);
    expect(screen.getByTestId('stats')).toBeInTheDocument();
    expect(screen.getByText('42')).toBeInTheDocument();
    expect(screen.getByText('$12.50')).toBeInTheDocument();
  });

  it('shows loading state', () => {
    render(<Stats loading={true} />);
    expect(screen.getByTestId('stats-loading')).toBeInTheDocument();
  });

  it('shows empty state', () => {
    render(<Stats stats={null} />);
    expect(screen.getByTestId('stats-empty')).toBeInTheDocument();
  });

  it('shows models used', () => {
    render(<Stats stats={mockStats} />);
    expect(screen.getByText('gpt-4')).toBeInTheDocument();
    expect(screen.getByText('30 sessions')).toBeInTheDocument();
  });

  it('formats large numbers', () => {
    render(<Stats stats={mockStats} />);
    expect(screen.getByText('1.5M')).toBeInTheDocument();
  });
});

describe('StatusNotices', () => {
  const notices = [
    { id: '1', type: 'info' as const, message: 'Info notice' },
    { id: '2', type: 'warning' as const, message: 'Warning notice', dismissible: true },
    { id: '3', type: 'error' as const, message: 'Error notice' },
  ];

  it('renders notices', () => {
    render(<StatusNotices notices={notices} />);
    expect(screen.getByTestId('status-notices')).toBeInTheDocument();
    expect(screen.getByText('Info notice')).toBeInTheDocument();
    expect(screen.getByText('Warning notice')).toBeInTheDocument();
  });

  it('returns null for empty notices', () => {
    const { container } = render(<StatusNotices notices={[]} />);
    expect(container.innerHTML).toBe('');
  });

  it('shows dismiss button for dismissible notices', () => {
    render(<StatusNotices notices={notices} onDismiss={vi.fn()} />);
    expect(screen.getByTestId('dismiss-notice-2')).toBeInTheDocument();
  });

  it('calls onDismiss when dismiss clicked', () => {
    const onDismiss = vi.fn();
    render(<StatusNotices notices={notices} onDismiss={onDismiss} />);
    fireEvent.click(screen.getByTestId('dismiss-notice-2'));
    expect(onDismiss).toHaveBeenCalledWith('2');
  });

  it('does not show dismiss for non-dismissible', () => {
    render(<StatusNotices notices={notices} onDismiss={vi.fn()} />);
    expect(screen.queryByTestId('dismiss-notice-1')).not.toBeInTheDocument();
  });
});

describe('ThinkingToggle', () => {
  it('renders with enabled state', () => {
    render(<ThinkingToggle currentValue={true} onSelect={vi.fn()} />);
    expect(screen.getByTestId('thinking-toggle')).toBeInTheDocument();
    expect(screen.getByText('Extended Thinking')).toBeInTheDocument();
  });

  it('calls onSelect with true when enable clicked', () => {
    const onSelect = vi.fn();
    render(<ThinkingToggle currentValue={false} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('thinking-enable'));
    expect(onSelect).toHaveBeenCalledWith(true);
  });

  it('calls onSelect with false when disable clicked', () => {
    const onSelect = vi.fn();
    render(<ThinkingToggle currentValue={true} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('thinking-disable'));
    expect(onSelect).toHaveBeenCalledWith(false);
  });

  it('shows mid-conversation warning', () => {
    render(<ThinkingToggle currentValue={true} onSelect={vi.fn()} isMidConversation={true} />);
    expect(screen.getByText(/Changing thinking mode mid-conversation/)).toBeInTheDocument();
  });

  it('shows cancel button when onCancel provided', () => {
    render(<ThinkingToggle currentValue={true} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByTestId('thinking-cancel')).toBeInTheDocument();
  });
});

describe('TokenWarning', () => {
  it('returns null when usage is normal', () => {
    const { container } = render(<TokenWarning tokenUsage={100} maxTokens={1000} />);
    expect(container.innerHTML).toBe('');
  });

  it('shows warning at 70% usage', () => {
    render(<TokenWarning tokenUsage={700} maxTokens={1000} />);
    expect(screen.getByTestId('token-warning')).toBeInTheDocument();
    expect(screen.getByText(/Token usage high/)).toBeInTheDocument();
  });

  it('shows critical at 90% usage', () => {
    render(<TokenWarning tokenUsage={950} maxTokens={1000} />);
    expect(screen.getByTestId('token-warning')).toBeInTheDocument();
    expect(screen.getByText(/Token usage critical/)).toBeInTheDocument();
  });

  it('shows model name', () => {
    render(<TokenWarning tokenUsage={800} maxTokens={1000} model="gpt-4" />);
    expect(screen.getByText('(gpt-4)')).toBeInTheDocument();
  });

  it('getWarningLevel works correctly', () => {
    expect(getWarningLevel(50, 1000)).toBe('normal');
    expect(getWarningLevel(700, 1000)).toBe('warning');
    expect(getWarningLevel(950, 1000)).toBe('critical');
  });
});

describe('FastIcon', () => {
  it('renders active icon', () => {
    render(<FastIcon />);
    expect(screen.getByTestId('fast-icon')).toBeInTheDocument();
  });

  it('renders cooldown icon', () => {
    render(<FastIcon cooldown={true} />);
    expect(screen.getByTestId('fast-icon')).toBeInTheDocument();
  });
});

describe('FilePathLink', () => {
  it('renders file path', () => {
    render(<FilePathLink filePath="/path/to/file.ts" />);
    expect(screen.getByTestId('file-path-link')).toBeInTheDocument();
    expect(screen.getByText('/path/to/file.ts')).toBeInTheDocument();
  });

  it('renders custom children', () => {
    render(<FilePathLink filePath="/path/to/file.ts">file.ts</FilePathLink>);
    expect(screen.getByText('file.ts')).toBeInTheDocument();
  });

  it('calls onClick when clicked', () => {
    const onClick = vi.fn();
    render(<FilePathLink filePath="/path/to/file.ts" onClick={onClick} />);
    fireEvent.click(screen.getByTestId('file-path-link'));
    expect(onClick).toHaveBeenCalledWith('/path/to/file.ts');
  });

  it('has title attribute with full path', () => {
    render(<FilePathLink filePath="/path/to/file.ts" />);
    expect(screen.getByTestId('file-path-link').getAttribute('title')).toBe('/path/to/file.ts');
  });
});
