import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { McpCapabilitiesSection } from './McpCapabilitiesSection';

describe('McpCapabilitiesSection', () => {
  beforeEach(() => cleanup());
  afterEach(() => cleanup());

  it('renders all four capability items', () => {
    render(<McpCapabilitiesSection capabilities={{ tools: true, resources: false, prompts: false, sampling: false }} />);
    expect(screen.getByText('工具')).toBeInTheDocument();
    expect(screen.getByText('资源')).toBeInTheDocument();
    expect(screen.getByText('提示')).toBeInTheDocument();
    expect(screen.getByText('采样')).toBeInTheDocument();
  });

  it('shows green check for supported capabilities', () => {
    render(<McpCapabilitiesSection capabilities={{ tools: true, resources: false, prompts: false, sampling: false }} />);
    const toolsEl = screen.getByTestId('mcp-capability-tools');
    expect(toolsEl).toHaveClass('bg-emerald-50');
  });

  it('shows grey cross for unsupported capabilities', () => {
    render(<McpCapabilitiesSection capabilities={{ tools: true, resources: false, prompts: false, sampling: false }} />);
    const resourcesEl = screen.getByTestId('mcp-capability-resources');
    expect(resourcesEl).toHaveClass('bg-slate-50');
  });

  it('handles all capabilities supported', () => {
    render(<McpCapabilitiesSection capabilities={{ tools: true, resources: true, prompts: true, sampling: true }} />);
    const container = screen.getByTestId('mcp-capabilities');
    const greenItems = container.querySelectorAll('.bg-emerald-50');
    expect(greenItems.length).toBe(4);
  });

  it('handles no capabilities supported', () => {
    render(<McpCapabilitiesSection capabilities={{ tools: false, resources: false, prompts: false, sampling: false }} />);
    const container = screen.getByTestId('mcp-capabilities');
    const greyItems = container.querySelectorAll('.bg-slate-50');
    expect(greyItems.length).toBe(4);
  });
});
