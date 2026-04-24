import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AgentEditor } from './AgentEditor';

const TOOLS = ['Bash', 'FileRead', 'FileEdit', 'Grep'];
const MODELS = [
  { id: 'claude-sonnet-4', name: 'Claude Sonnet 4', provider: 'Anthropic' },
];

const EXISTING_AGENT = {
  name: 'coder',
  description: '代码助手',
  model: 'claude-sonnet-4',
  color: '#3b82f6',
  system_prompt: '你是一个代码专家',
  tools: ['Bash', 'FileEdit'],
  disabled: false,
};

describe('AgentEditor', () => {
  afterEach(cleanup);

  it('renders create mode with empty fields', () => {
    render(<AgentEditor onSave={vi.fn()} onCancel={vi.fn()} availableTools={TOOLS} />);
    expect(screen.getByText('创建 Agent')).toBeInTheDocument();
    expect(screen.getByLabelText('名称 *')).toHaveValue('');
    expect(screen.getByLabelText('系统提示词 *')).toHaveValue('');
  });

  it('renders edit mode with pre-filled fields', () => {
    render(
      <AgentEditor
        agent={EXISTING_AGENT}
        onSave={vi.fn()}
        onCancel={vi.fn()}
        availableTools={TOOLS}
      />,
    );
    expect(screen.getByText('编辑 Agent: coder')).toBeInTheDocument();
    expect(screen.getByLabelText('名称 *')).toHaveValue('coder');
    expect(screen.getByLabelText('系统提示词 *')).toHaveValue('你是一个代码专家');
  });

  it('makes name readonly in edit mode', () => {
    render(
      <AgentEditor
        agent={EXISTING_AGENT}
        onSave={vi.fn()}
        onCancel={vi.fn()}
        availableTools={TOOLS}
      />,
    );
    expect(screen.getByLabelText('名称 *')).toHaveAttribute('readonly');
  });

  it('shows validation errors for empty required fields', () => {
    render(<AgentEditor onSave={vi.fn()} onCancel={vi.fn()} availableTools={TOOLS} />);
    fireEvent.click(screen.getByText('创建'));
    expect(screen.getByText('名称不能为空')).toBeInTheDocument();
    expect(screen.getByText('系统提示词不能为空')).toBeInTheDocument();
  });

  it('calls onSave with form data', () => {
    const onSave = vi.fn();
    render(<AgentEditor onSave={onSave} onCancel={vi.fn()} availableTools={TOOLS} models={MODELS} />);
    fireEvent.change(screen.getByLabelText('名称 *'), { target: { value: 'my-agent' } });
    fireEvent.change(screen.getByLabelText('描述'), { target: { value: '测试 agent' } });
    fireEvent.change(screen.getByLabelText('系统提示词 *'), { target: { value: '你好' } });
    fireEvent.click(screen.getByText('创建'));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'my-agent',
        description: '测试 agent',
        system_prompt: '你好',
        tools: [],
        disabled: false,
      }),
    );
  });

  it('calls onCancel when cancel button is clicked', () => {
    const onCancel = vi.fn();
    render(<AgentEditor onSave={vi.fn()} onCancel={onCancel} availableTools={TOOLS} />);
    fireEvent.click(screen.getByText('取消'));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('toggles tools via checkbox', () => {
    const onSave = vi.fn();
    render(<AgentEditor onSave={onSave} onCancel={vi.fn()} availableTools={TOOLS} />);
    fireEvent.change(screen.getByLabelText('名称 *'), { target: { value: 'test' } });
    fireEvent.change(screen.getByLabelText('系统提示词 *'), { target: { value: 'prompt' } });
    fireEvent.click(screen.getByText('Bash'));
    fireEvent.click(screen.getByText('创建'));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ tools: ['Bash'] }),
    );
  });
});
