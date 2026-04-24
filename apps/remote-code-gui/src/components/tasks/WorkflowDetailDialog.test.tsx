import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { WorkflowDetailDialog } from './WorkflowDetailDialog';

const steps = [
  { name: 'Checkout', status: 'completed' },
  { name: 'Build', status: 'running' },
  { name: 'Test', status: 'pending' },
];

describe('WorkflowDetailDialog', () => {
  afterEach(cleanup);

  it('returns null when visible is false', () => {
    render(
      <WorkflowDetailDialog
        visible={false}
        workflowName="CI"
        steps={steps}
        onClose={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('workflow-detail-dialog')).toBeNull();
  });

  it('renders dialog with workflow name', () => {
    render(
      <WorkflowDetailDialog
        visible={true}
        workflowName="CI Pipeline"
        steps={steps}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByTestId('workflow-detail-dialog')).toBeInTheDocument();
    expect(screen.getByText('CI Pipeline')).toBeInTheDocument();
  });

  it('shows all step names', () => {
    render(
      <WorkflowDetailDialog
        visible={true}
        workflowName="CI"
        steps={steps}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText('Checkout')).toBeInTheDocument();
    expect(screen.getByText('Build')).toBeInTheDocument();
    expect(screen.getByText('Test')).toBeInTheDocument();
  });

  it('shows step status text', () => {
    render(
      <WorkflowDetailDialog
        visible={true}
        workflowName="CI"
        steps={steps}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText('completed')).toBeInTheDocument();
    expect(screen.getByText('running')).toBeInTheDocument();
    expect(screen.getByText('pending')).toBeInTheDocument();
  });

  it('shows empty message when no steps', () => {
    render(
      <WorkflowDetailDialog
        visible={true}
        workflowName="CI"
        steps={[]}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText('暂无步骤')).toBeInTheDocument();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    render(
      <WorkflowDetailDialog
        visible={true}
        workflowName="CI"
        steps={steps}
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByLabelText('关闭'));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('applies custom className', () => {
    render(
      <WorkflowDetailDialog
        visible={true}
        workflowName="CI"
        steps={steps}
        onClose={vi.fn()}
        className="my-cls"
      />,
    );
    expect(screen.getByTestId('workflow-detail-dialog').className).toContain('my-cls');
  });

  it('shows failed step with red styling', () => {
    const failedSteps = [{ name: 'Deploy', status: 'failed' }];
    render(
      <WorkflowDetailDialog
        visible={true}
        workflowName="CI"
        steps={failedSteps}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText('failed')).toBeInTheDocument();
  });
});
