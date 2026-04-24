import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AdvisorMessage, type AdvisorBlock } from './AdvisorMessage';

describe('AdvisorMessage', () => {
  afterEach(() => { cleanup(); });

  it('renders with data-testid', () => {
    render(<AdvisorMessage content="Test message" />);
    expect(screen.getByTestId('advisor-message')).toBeInTheDocument();
  });

  it('renders content text', () => {
    render(<AdvisorMessage content="Hello advisor" />);
    expect(screen.getByText('Hello advisor')).toBeInTheDocument();
  });

  it('renders default sender', () => {
    render(<AdvisorMessage content="Test" />);
    expect(screen.getByText('Advisor')).toBeInTheDocument();
  });

  it('renders custom sender', () => {
    render(<AdvisorMessage content="Test" sender="Custom Bot" />);
    expect(screen.getByText('Custom Bot')).toBeInTheDocument();
  });

  it('renders timestamp when provided', () => {
    render(<AdvisorMessage content="Test" timestamp="12:00" />);
    expect(screen.getByText('12:00')).toBeInTheDocument();
  });

  it('renders model name when provided', () => {
    render(<AdvisorMessage content="Test" modelName="gpt-4" />);
    expect(screen.getByText('gpt-4')).toBeInTheDocument();
  });

  it('renders server_tool_use block with loading state', () => {
    const block: AdvisorBlock = {
      type: 'server_tool_use',
      id: 'tool-1',
      input: { command: 'ls' },
    };
    render(<AdvisorMessage content="" block={block} isLoading={true} />);
    expect(screen.getByTestId('advisor-tool-use')).toBeInTheDocument();
    expect(screen.getByTestId('advisor-tool-loading')).toBeInTheDocument();
    expect(screen.getByText('Advising')).toBeInTheDocument();
  });

  it('renders server_tool_use block with model name', () => {
    const block: AdvisorBlock = {
      type: 'server_tool_use',
      id: 'tool-1',
    };
    render(<AdvisorMessage content="" block={block} modelName="claude-3" />);
    expect(screen.getByText('claude-3')).toBeInTheDocument();
  });

  it('renders tool input toggle for tool use with input', () => {
    const block: AdvisorBlock = {
      type: 'server_tool_use',
      id: 'tool-1',
      input: { command: 'ls -la' },
    };
    render(<AdvisorMessage content="" block={block} />);
    expect(screen.getByTestId('advisor-tool-input-toggle')).toBeInTheDocument();
  });

  it('expands tool input on toggle click', () => {
    const block: AdvisorBlock = {
      type: 'server_tool_use',
      id: 'tool-1',
      input: { command: 'ls' },
    };
    render(<AdvisorMessage content="" block={block} />);
    fireEvent.click(screen.getByTestId('advisor-tool-input-toggle'));
    // Should show JSON input
    expect(screen.getByText(/"command"/)).toBeInTheDocument();
  });

  it('renders tool_result_error block', () => {
    const block: AdvisorBlock = {
      type: 'tool_result_error',
      error_code: 'RATE_LIMIT',
    };
    render(<AdvisorMessage content="" block={block} />);
    expect(screen.getByTestId('advisor-error')).toBeInTheDocument();
    expect(screen.getByText('Advisor 不可用')).toBeInTheDocument();
    expect(screen.getByText(/RATE_LIMIT/)).toBeInTheDocument();
  });

  it('renders advisor_result block in non-verbose mode', () => {
    const block: AdvisorBlock = {
      type: 'advisor_result',
      text: 'This is the advisor feedback text',
    };
    render(<AdvisorMessage content="" block={block} verbose={false} />);
    expect(screen.getByTestId('advisor-result')).toBeInTheDocument();
    expect(screen.getByText('Advisor 已审查对话并将应用反馈')).toBeInTheDocument();
  });

  it('renders advisor_result block in verbose mode', () => {
    const block: AdvisorBlock = {
      type: 'advisor_result',
      text: 'Verbose feedback text here',
    };
    render(<AdvisorMessage content="" block={block} verbose={true} />);
    expect(screen.getByTestId('advisor-result-verbose')).toBeInTheDocument();
    expect(screen.getByText('Verbose feedback text here')).toBeInTheDocument();
  });

  it('renders advisor_redacted_result block', () => {
    const block: AdvisorBlock = {
      type: 'advisor_redacted_result',
    };
    render(<AdvisorMessage content="" block={block} />);
    expect(screen.getByTestId('advisor-redacted')).toBeInTheDocument();
    expect(screen.getByText('Advisor 已审查对话并将应用反馈')).toBeInTheDocument();
  });

  it('renders resolved tool use with check icon', () => {
    const block: AdvisorBlock = {
      type: 'server_tool_use',
      id: 'tool-1',
    };
    const resolvedIDs = new Set(['tool-1']);
    render(<AdvisorMessage content="" block={block} resolvedToolUseIDs={resolvedIDs} />);
    expect(screen.getByTestId('advisor-tool-use')).toBeInTheDocument();
    // Should not show loading
    expect(screen.queryByTestId('advisor-tool-loading')).not.toBeInTheDocument();
  });

  it('renders errored tool use with error icon', () => {
    const block: AdvisorBlock = {
      type: 'server_tool_use',
      id: 'tool-1',
    };
    const erroredIDs = new Set(['tool-1']);
    render(<AdvisorMessage content="" block={block} erroredToolUseIDs={erroredIDs} />);
    expect(screen.getByTestId('advisor-tool-error')).toBeInTheDocument();
  });

  it('renders text block type with content', () => {
    const block: AdvisorBlock = {
      type: 'text',
      text: 'Simple text content',
    };
    render(<AdvisorMessage content="" block={block} />);
    expect(screen.getByText('Simple text content')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    const { container } = render(<AdvisorMessage content="Test" className="custom-class" />);
    const el = container.firstChild as HTMLElement;
    expect(el.classList.contains('custom-class')).toBe(true);
  });
});
