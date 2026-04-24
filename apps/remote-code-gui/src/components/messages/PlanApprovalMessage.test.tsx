import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PlanApprovalMessage } from './PlanApprovalMessage';

describe('PlanApprovalMessage', () => {
  afterEach(cleanup);

  it('渲染 request 变体', () => {
    render(
      <PlanApprovalMessage variant="request" from="alice" planContent="重构模块" />,
    );
    expect(screen.getByTestId('plan-approval-message')).toBeInTheDocument();
    expect(screen.getByText('Plan approval request from alice')).toBeInTheDocument();
    expect(screen.getByText('重构模块')).toBeInTheDocument();
  });

  it('request 变体使用蓝色虚线边框', () => {
    const { container } = render(
      <PlanApprovalMessage variant="request" from="alice" />,
    );
    expect(container.firstChild).toHaveClass('border-dashed');
    expect(container.firstChild).toHaveClass('border-blue-300');
  });

  it('显示计划文件路径', () => {
    render(
      <PlanApprovalMessage
        variant="request"
        from="alice"
        planFilePath="/plans/plan-1.md"
      />,
    );
    expect(screen.getByText('/plans/plan-1.md')).toBeInTheDocument();
  });

  it('渲染 approved response 变体', () => {
    render(<PlanApprovalMessage variant="response" from="bob" approved />);
    expect(screen.getByText('Plan approved by bob')).toBeInTheDocument();
  });

  it('approved response 使用绿色样式', () => {
    const { container } = render(
      <PlanApprovalMessage variant="response" from="bob" approved />,
    );
    expect(container.firstChild).toHaveClass('border-emerald-300');
  });

  it('渲染 rejected response 变体', () => {
    render(
      <PlanApprovalMessage
        variant="response"
        from="bob"
        approved={false}
        reason="风险太高"
      />,
    );
    expect(screen.getByText('Plan rejected by bob')).toBeInTheDocument();
    expect(screen.getByText('风险太高')).toBeInTheDocument();
  });

  it('rejected response 使用红色样式', () => {
    const { container } = render(
      <PlanApprovalMessage variant="response" from="bob" approved={false} />,
    );
    expect(container.firstChild).toHaveClass('border-red-300');
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <PlanApprovalMessage
        variant="request"
        from="alice"
        className="plan-custom"
      />,
    );
    expect(container.firstChild).toHaveClass('plan-custom');
  });
});
