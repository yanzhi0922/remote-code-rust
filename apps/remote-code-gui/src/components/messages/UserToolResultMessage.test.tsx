import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import {
  UserToolSuccessMessage,
  UserToolErrorMessage,
  UserToolRejectMessage,
  UserToolCanceledMessage,
  RejectedToolUseMessage,
  RejectedPlanMessage,
  UserToolResultMessage,
} from './UserToolResultMessage';
import type { ConversationEntry } from '../../lib/types';

describe('UserToolSuccessMessage', () => {
  afterEach(cleanup);

  it('渲染成功结果', () => {
    render(<UserToolSuccessMessage toolName="Read" output="文件内容正常" />);
    expect(screen.getByText('工具成功')).toBeInTheDocument();
    expect(screen.getByText('Read')).toBeInTheDocument();
    expect(screen.getByText('文件内容正常')).toBeInTheDocument();
  });

  it('使用绿色边框样式', () => {
    const { container } = render(
      <UserToolSuccessMessage toolName="Bash" output="ok" />,
    );
    expect(container.firstChild).toHaveClass('border-emerald-200');
  });
});

describe('UserToolErrorMessage', () => {
  afterEach(cleanup);

  it('渲染错误结果', () => {
    render(<UserToolErrorMessage toolName="Bash" output="命令执行失败" />);
    expect(screen.getByText('工具错误')).toBeInTheDocument();
    expect(screen.getByText('Bash')).toBeInTheDocument();
    expect(screen.getByText('命令执行失败')).toBeInTheDocument();
  });

  it('使用红色边框样式', () => {
    const { container } = render(
      <UserToolErrorMessage toolName="Bash" output="err" />,
    );
    expect(container.firstChild).toHaveClass('border-rose-200');
  });
});

describe('UserToolRejectMessage', () => {
  afterEach(cleanup);

  it('渲染拒绝结果', () => {
    render(
      <UserToolRejectMessage
        toolName="Write"
        output="用户拒绝了写入操作"
        reason="安全策略限制"
      />,
    );
    expect(screen.getByText('工具被拒绝')).toBeInTheDocument();
    expect(screen.getByText('安全策略限制')).toBeInTheDocument();
  });

  it('无 reason 时不显示原因', () => {
    render(<UserToolRejectMessage toolName="Write" output="已拒绝" />);
    expect(screen.getByText('工具被拒绝')).toBeInTheDocument();
  });
});

describe('UserToolCanceledMessage', () => {
  afterEach(cleanup);

  it('渲染取消结果', () => {
    render(<UserToolCanceledMessage toolName="Grep" output="" />);
    expect(screen.getByText('工具已取消')).toBeInTheDocument();
  });
});

describe('RejectedToolUseMessage', () => {
  afterEach(cleanup);

  it('渲染工具使用被拒绝', () => {
    render(
      <RejectedToolUseMessage toolName="Bash" input={{ command: 'rm -rf /' }} />,
    );
    expect(screen.getByText('工具使用被拒绝')).toBeInTheDocument();
    expect(screen.getByText('Bash')).toBeInTheDocument();
  });

  it('支持字符串输入', () => {
    render(<RejectedToolUseMessage toolName="Read" input="some string" />);
    expect(screen.getByText('some string')).toBeInTheDocument();
  });
});

describe('RejectedPlanMessage', () => {
  afterEach(cleanup);

  it('渲染计划被拒绝', () => {
    render(<RejectedPlanMessage planContent="重构整个模块" />);
    expect(screen.getByText('计划被拒绝')).toBeInTheDocument();
    expect(screen.getByText('重构整个模块')).toBeInTheDocument();
  });
});

describe('UserToolResultMessage（统一入口）', () => {
  afterEach(cleanup);

  it('成功时渲染成功样式', () => {
    const entry: ConversationEntry = {
      role: 'tool',
      text: '文件已读取',
      content_blocks: [],
      tool_calls: [],
      tool_call_id: 'tc-1',
      name: 'Read',
      is_error: false,
    };
    render(<UserToolResultMessage entry={entry} />);
    expect(screen.getByText('工具成功')).toBeInTheDocument();
  });

  it('错误时渲染错误样式', () => {
    const entry: ConversationEntry = {
      role: 'tool',
      text: '文件不存在',
      content_blocks: [],
      tool_calls: [],
      tool_call_id: 'tc-2',
      name: 'Read',
      is_error: true,
    };
    render(<UserToolResultMessage entry={entry} />);
    expect(screen.getByText('工具错误')).toBeInTheDocument();
  });
});
