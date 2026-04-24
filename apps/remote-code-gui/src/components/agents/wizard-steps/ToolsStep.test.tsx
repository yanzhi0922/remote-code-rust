import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ToolsStep } from './ToolsStep';

const TOOLS = ['Bash', 'FileEdit', 'FileRead'];

describe('ToolsStep', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<ToolsStep selected={[]} onToggle={vi.fn()} availableTools={TOOLS} />);
    expect(screen.getByTestId('wizard-tools-step')).toBeInTheDocument();
  });

  it('renders all available tools', () => {
    render(<ToolsStep selected={[]} onToggle={vi.fn()} availableTools={TOOLS} />);
    expect(screen.getByTestId('tool-item-Bash')).toBeInTheDocument();
    expect(screen.getByTestId('tool-item-FileEdit')).toBeInTheDocument();
    expect(screen.getByTestId('tool-item-FileRead')).toBeInTheDocument();
  });

  it('calls onToggle when a tool checkbox is changed', () => {
    const onToggle = vi.fn();
    render(<ToolsStep selected={[]} onToggle={onToggle} availableTools={TOOLS} />);
    fireEvent.click(screen.getByTestId('tool-checkbox-Bash'));
    expect(onToggle).toHaveBeenCalledWith('Bash');
  });

  it('shows selected tools with blue styling', () => {
    render(<ToolsStep selected={['Bash']} onToggle={vi.fn()} availableTools={TOOLS} />);
    const bashItem = screen.getByTestId('tool-item-Bash');
    expect(bashItem.className).toContain('border-blue-500');
  });

  it('shows tool count', () => {
    render(<ToolsStep selected={['Bash']} onToggle={vi.fn()} availableTools={TOOLS} />);
    expect(screen.getByText('已选择 1 / 3 个工具')).toBeInTheDocument();
  });

  it('selects all tools when select-all is clicked', () => {
    const onToggle = vi.fn();
    render(<ToolsStep selected={[]} onToggle={onToggle} availableTools={TOOLS} />);
    fireEvent.click(screen.getByTestId('select-all-tools'));
    expect(onToggle).toHaveBeenCalledTimes(3);
    expect(onToggle).toHaveBeenCalledWith('Bash');
    expect(onToggle).toHaveBeenCalledWith('FileEdit');
    expect(onToggle).toHaveBeenCalledWith('FileRead');
  });

  it('shows empty state when no tools available', () => {
    render(<ToolsStep selected={[]} onToggle={vi.fn()} availableTools={[]} />);
    expect(screen.getByTestId('no-tools')).toBeInTheDocument();
  });

  it('deselects all when all are selected and select-all is clicked', () => {
    const onToggle = vi.fn();
    render(<ToolsStep selected={TOOLS} onToggle={onToggle} availableTools={TOOLS} />);
    fireEvent.click(screen.getByTestId('select-all-tools'));
    expect(onToggle).toHaveBeenCalledTimes(3);
  });

  it('applies custom className', () => {
    render(<ToolsStep selected={[]} onToggle={vi.fn()} availableTools={TOOLS} className="extra" />);
    expect(screen.getByTestId('wizard-tools-step').className).toContain('extra');
  });
});
