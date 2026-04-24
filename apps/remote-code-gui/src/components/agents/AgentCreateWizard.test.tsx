import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AgentCreateWizard } from './AgentCreateWizard';

const TOOLS = ['Bash', 'FileRead', 'FileEdit', 'Grep'];
const MODELS = [
  { id: 'claude-sonnet-4', name: 'Claude Sonnet 4', provider: 'Anthropic' },
];

describe('AgentCreateWizard', () => {
  afterEach(cleanup);

  it('renders step 1 (basic info) by default', () => {
    render(<AgentCreateWizard onComplete={vi.fn()} onCancel={vi.fn()} availableTools={TOOLS} />);
    expect(screen.getByText('基本信息')).toBeInTheDocument();
    expect(screen.getByLabelText('名称 *')).toBeInTheDocument();
    expect(screen.getByText('模型和行为')).toBeInTheDocument();
    expect(screen.getByText('工具选择')).toBeInTheDocument();
  });

  it('shows validation error when name is empty on next', () => {
    render(<AgentCreateWizard onComplete={vi.fn()} onCancel={vi.fn()} availableTools={TOOLS} />);
    fireEvent.click(screen.getByText('下一步'));
    expect(screen.getByText('名称不能为空')).toBeInTheDocument();
  });

  it('advances to step 2 when name is filled', () => {
    render(<AgentCreateWizard onComplete={vi.fn()} onCancel={vi.fn()} availableTools={TOOLS} />);
    fireEvent.change(screen.getByLabelText('名称 *'), { target: { value: 'my-agent' } });
    fireEvent.click(screen.getByText('下一步'));
    expect(screen.getByLabelText('系统提示词 *')).toBeInTheDocument();
  });

  it('shows validation error when system prompt is empty on step 2', () => {
    render(<AgentCreateWizard onComplete={vi.fn()} onCancel={vi.fn()} availableTools={TOOLS} />);
    fireEvent.change(screen.getByLabelText('名称 *'), { target: { value: 'test' } });
    fireEvent.click(screen.getByText('下一步'));
    fireEvent.click(screen.getByText('下一步'));
    expect(screen.getByText('系统提示词不能为空')).toBeInTheDocument();
  });

  it('advances to step 3 and shows tools', () => {
    render(<AgentCreateWizard onComplete={vi.fn()} onCancel={vi.fn()} availableTools={TOOLS} />);
    fireEvent.change(screen.getByLabelText('名称 *'), { target: { value: 'test' } });
    fireEvent.click(screen.getByText('下一步'));
    fireEvent.change(screen.getByLabelText('系统提示词 *'), { target: { value: 'hello' } });
    fireEvent.click(screen.getByText('下一步'));
    expect(screen.getByText('Bash')).toBeInTheDocument();
    expect(screen.getByText('FileRead')).toBeInTheDocument();
  });

  it('calls onComplete with form data when finish is clicked', () => {
    const onComplete = vi.fn();
    render(<AgentCreateWizard onComplete={onComplete} onCancel={vi.fn()} availableTools={TOOLS} models={MODELS} />);
    // Step 1
    fireEvent.change(screen.getByLabelText('名称 *'), { target: { value: 'my-agent' } });
    fireEvent.change(screen.getByLabelText('描述'), { target: { value: '测试' } });
    fireEvent.click(screen.getByText('下一步'));
    // Step 2
    fireEvent.change(screen.getByLabelText('系统提示词 *'), { target: { value: '你是一个助手' } });
    fireEvent.click(screen.getByText('下一步'));
    // Step 3
    fireEvent.click(screen.getByText('Bash'));
    fireEvent.click(screen.getByText('完成'));
    expect(onComplete).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'my-agent',
        description: '测试',
        system_prompt: '你是一个助手',
        tools: ['Bash'],
        disabled: false,
      }),
    );
  });

  it('goes back to previous step', () => {
    render(<AgentCreateWizard onComplete={vi.fn()} onCancel={vi.fn()} availableTools={TOOLS} />);
    fireEvent.change(screen.getByLabelText('名称 *'), { target: { value: 'test' } });
    fireEvent.click(screen.getByText('下一步'));
    expect(screen.getByLabelText('系统提示词 *')).toBeInTheDocument();
    fireEvent.click(screen.getByText('上一步'));
    expect(screen.getByLabelText('名称 *')).toBeInTheDocument();
  });

  it('calls onCancel from step 1', () => {
    const onCancel = vi.fn();
    render(<AgentCreateWizard onComplete={vi.fn()} onCancel={onCancel} availableTools={TOOLS} />);
    fireEvent.click(screen.getByText('取消'));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
