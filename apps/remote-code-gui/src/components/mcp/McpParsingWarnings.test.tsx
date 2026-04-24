import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { McpParsingWarnings } from './McpParsingWarnings';

describe('McpParsingWarnings', () => {
  beforeEach(() => cleanup());
  afterEach(() => cleanup());

  it('renders nothing when warnings array is empty', () => {
    render(<McpParsingWarnings warnings={[]} />);
    expect(screen.queryByTestId('mcp-parsing-warnings')).not.toBeInTheDocument();
  });

  it('renders all warnings', () => {
    const warnings = ['警告一', '警告二', '警告三'];
    render(<McpParsingWarnings warnings={warnings} />);
    expect(screen.getByText('警告一')).toBeInTheDocument();
    expect(screen.getByText('警告二')).toBeInTheDocument();
    expect(screen.getByText('警告三')).toBeInTheDocument();
  });

  it('shows warning count in header', () => {
    const warnings = ['a', 'b'];
    render(<McpParsingWarnings warnings={warnings} />);
    expect(screen.getByText(/配置警告 \(2\)/)).toBeInTheDocument();
  });

  it('is expanded by default', () => {
    const warnings = ['test warning'];
    render(<McpParsingWarnings warnings={warnings} />);
    expect(screen.getByText('test warning')).toBeInTheDocument();
  });

  it('collapses when toggle is clicked', () => {
    const warnings = ['test warning'];
    render(<McpParsingWarnings warnings={warnings} />);
    fireEvent.click(screen.getByTestId('mcp-parsing-warnings-toggle'));
    expect(screen.queryByText('test warning')).not.toBeInTheDocument();
  });

  it('expands again when toggle clicked twice', () => {
    const warnings = ['test warning'];
    render(<McpParsingWarnings warnings={warnings} />);
    fireEvent.click(screen.getByTestId('mcp-parsing-warnings-toggle'));
    fireEvent.click(screen.getByTestId('mcp-parsing-warnings-toggle'));
    expect(screen.getByText('test warning')).toBeInTheDocument();
  });
});
