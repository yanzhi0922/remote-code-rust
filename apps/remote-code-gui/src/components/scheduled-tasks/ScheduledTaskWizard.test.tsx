import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { ScheduledTaskWizard } from './ScheduledTaskWizard';
import { NameStep } from './steps/NameStep';
import { TaskDescriptionStep } from './steps/TaskDescriptionStep';
import { ScheduleStep } from './steps/ScheduleStep';
import { FolderStep } from './steps/FolderStep';
import { TaskPromptStep } from './steps/TaskPromptStep';
import { TaskModelStep } from './steps/TaskModelStep';
import { PermissionStep } from './steps/PermissionStep';
import { TaskConfirmStep } from './steps/TaskConfirmStep';
import { WizardProvider } from '../wizard';

afterEach(() => {
  cleanup();
});

// Helper: wraps a step in WizardProvider for isolated testing
function StepWrapper({ children, initialData = {} }: { children: React.ReactNode; initialData?: Record<string, unknown> }) {
  return (
    <WizardProvider steps={[<div key="1" />, <div key="2" />]} initialData={initialData} onComplete={vi.fn()}>
      {children}
    </WizardProvider>
  );
}

// ─── ScheduledTaskWizard ─────────────────────────────────────────────

describe('ScheduledTaskWizard', () => {
  it('renders wizard in create mode', () => {
    render(<ScheduledTaskWizard mode="create" onComplete={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByTestId('scheduled-task-wizard')).toBeInTheDocument();
    expect(screen.getByTestId('scheduled-task-wizard-steps')).toBeInTheDocument();
  });

  it('renders wizard in edit mode', () => {
    render(<ScheduledTaskWizard mode="edit" onComplete={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByTestId('scheduled-task-wizard')).toBeInTheDocument();
  });

  it('calls onCancel when back is pressed on first step', () => {
    const onCancel = vi.fn();
    render(<ScheduledTaskWizard mode="create" onComplete={vi.fn()} onCancel={onCancel} />);
    fireEvent.click(screen.getByTestId('wizard-cancel-btn'));
    expect(onCancel).toHaveBeenCalled();
  });

  it('shows first step (NameStep) initially', () => {
    render(<ScheduledTaskWizard mode="create" onComplete={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByTestId('name-input')).toBeInTheDocument();
  });

  it('renders with initial data', () => {
    render(
      <ScheduledTaskWizard
        mode="edit"
        initialData={{ name: 'my-task' }}
        onComplete={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByTestId('name-input')).toHaveValue('my-task');
  });
});

// ─── NameStep ────────────────────────────────────────────────────────

describe('NameStep', () => {
  it('renders input and buttons', () => {
    render(<StepWrapper><NameStep /></StepWrapper>);
    expect(screen.getByTestId('name-input')).toBeInTheDocument();
    expect(screen.getByTestId('name-submit')).toBeInTheDocument();
    expect(screen.getByTestId('name-back')).toBeInTheDocument();
  });

  it('shows error when submitting empty name', () => {
    render(<StepWrapper><NameStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('name-submit'));
    expect(screen.getByTestId('name-error')).toHaveTextContent('Name is required');
  });

  it('accepts valid name input', () => {
    render(<StepWrapper><NameStep /></StepWrapper>);
    fireEvent.change(screen.getByTestId('name-input'), { target: { value: 'daily-review' } });
    expect(screen.getByTestId('name-input')).toHaveValue('daily-review');
  });

  it('clears error on input change', () => {
    render(<StepWrapper><NameStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('name-submit'));
    expect(screen.getByTestId('name-error')).toBeInTheDocument();
    fireEvent.change(screen.getByTestId('name-input'), { target: { value: 'x' } });
    expect(screen.queryByTestId('name-error')).not.toBeInTheDocument();
  });

  it('submits on Enter key', () => {
    render(<StepWrapper><NameStep /></StepWrapper>);
    fireEvent.change(screen.getByTestId('name-input'), { target: { value: 'test-task' } });
    fireEvent.keyDown(screen.getByTestId('name-input'), { key: 'Enter' });
    // Should have moved forward (no error)
    expect(screen.queryByTestId('name-error')).not.toBeInTheDocument();
  });
});

// ─── TaskDescriptionStep ─────────────────────────────────────────────

describe('TaskDescriptionStep', () => {
  it('renders input and buttons', () => {
    render(<StepWrapper><TaskDescriptionStep /></StepWrapper>);
    expect(screen.getByTestId('description-input')).toBeInTheDocument();
  });

  it('shows error when submitting empty description', () => {
    render(<StepWrapper><TaskDescriptionStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('description-submit'));
    expect(screen.getByTestId('description-error')).toHaveTextContent('Description is required');
  });

  it('accepts valid description input', () => {
    render(<StepWrapper><TaskDescriptionStep /></StepWrapper>);
    fireEvent.change(screen.getByTestId('description-input'), { target: { value: 'A test task' } });
    expect(screen.getByTestId('description-input')).toHaveValue('A test task');
  });

  it('clears error on input change', () => {
    render(<StepWrapper><TaskDescriptionStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('description-submit'));
    expect(screen.getByTestId('description-error')).toBeInTheDocument();
    fireEvent.change(screen.getByTestId('description-input'), { target: { value: 'x' } });
    expect(screen.queryByTestId('description-error')).not.toBeInTheDocument();
  });

  it('submits on Enter key', () => {
    render(<StepWrapper><TaskDescriptionStep /></StepWrapper>);
    fireEvent.change(screen.getByTestId('description-input'), { target: { value: 'test desc' } });
    fireEvent.keyDown(screen.getByTestId('description-input'), { key: 'Enter' });
    expect(screen.queryByTestId('description-error')).not.toBeInTheDocument();
  });
});

// ─── TaskPromptStep ──────────────────────────────────────────────────

describe('TaskPromptStep', () => {
  it('renders textarea and buttons', () => {
    render(<StepWrapper><TaskPromptStep /></StepWrapper>);
    expect(screen.getByTestId('prompt-input')).toBeInTheDocument();
  });

  it('shows error when submitting empty prompt', () => {
    render(<StepWrapper><TaskPromptStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('prompt-submit'));
    expect(screen.getByTestId('prompt-error')).toHaveTextContent('Prompt is required');
  });

  it('accepts valid prompt input', () => {
    render(<StepWrapper><TaskPromptStep /></StepWrapper>);
    fireEvent.change(screen.getByTestId('prompt-input'), { target: { value: 'Review code' } });
    expect(screen.getByTestId('prompt-input')).toHaveValue('Review code');
  });

  it('clears error on input change', () => {
    render(<StepWrapper><TaskPromptStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('prompt-submit'));
    expect(screen.getByTestId('prompt-error')).toBeInTheDocument();
    fireEvent.change(screen.getByTestId('prompt-input'), { target: { value: 'x' } });
    expect(screen.queryByTestId('prompt-error')).not.toBeInTheDocument();
  });

  it('uses textarea element', () => {
    render(<StepWrapper><TaskPromptStep /></StepWrapper>);
    const el = screen.getByTestId('prompt-input');
    expect(el.tagName).toBe('TEXTAREA');
  });
});

// ─── TaskModelStep ───────────────────────────────────────────────────

describe('TaskModelStep', () => {
  it('renders model options', () => {
    render(<StepWrapper><TaskModelStep /></StepWrapper>);
    expect(screen.getByTestId('model-options')).toBeInTheDocument();
    expect(screen.getByTestId('model-default')).toBeInTheDocument();
    expect(screen.getByTestId('model-claude-sonnet')).toBeInTheDocument();
  });

  it('selects a model on click', () => {
    render(<StepWrapper><TaskModelStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('model-claude-opus'));
    expect(screen.getByTestId('model-claude-opus')).toHaveClass('bg-blue-100');
  });

  it('renders back and submit buttons', () => {
    render(<StepWrapper><TaskModelStep /></StepWrapper>);
    expect(screen.getByTestId('model-back')).toBeInTheDocument();
    expect(screen.getByTestId('model-submit')).toBeInTheDocument();
  });

  it('defaults to default model', () => {
    render(<StepWrapper><TaskModelStep /></StepWrapper>);
    expect(screen.getByTestId('model-default')).toHaveClass('bg-blue-100');
  });

  it('uses initial model from wizard data', () => {
    render(
      <StepWrapper initialData={{ model: 'claude-haiku' }}>
        <TaskModelStep />
      </StepWrapper>,
    );
    expect(screen.getByTestId('model-claude-haiku')).toHaveClass('bg-blue-100');
  });
});

// ─── PermissionStep ──────────────────────────────────────────────────

describe('PermissionStep', () => {
  it('renders permission options', () => {
    render(<StepWrapper><PermissionStep /></StepWrapper>);
    expect(screen.getByTestId('permission-options')).toBeInTheDocument();
    expect(screen.getByTestId('perm-ask')).toBeInTheDocument();
    expect(screen.getByTestId('perm-bypass')).toBeInTheDocument();
  });

  it('selects a permission mode on click', () => {
    render(<StepWrapper><PermissionStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('perm-bypass'));
    expect(screen.getByTestId('perm-bypass')).toHaveClass('bg-blue-100');
  });

  it('defaults to ask mode', () => {
    render(<StepWrapper><PermissionStep /></StepWrapper>);
    expect(screen.getByTestId('perm-ask')).toHaveClass('bg-blue-100');
  });

  it('renders back and submit buttons', () => {
    render(<StepWrapper><PermissionStep /></StepWrapper>);
    expect(screen.getByTestId('perm-back')).toBeInTheDocument();
    expect(screen.getByTestId('perm-submit')).toBeInTheDocument();
  });

  it('uses initial permission from wizard data', () => {
    render(
      <StepWrapper initialData={{ permissionMode: 'plan' }}>
        <PermissionStep />
      </StepWrapper>,
    );
    expect(screen.getByTestId('perm-plan')).toHaveClass('bg-blue-100');
  });
});

// ─── ScheduleStep ────────────────────────────────────────────────────

describe('ScheduleStep', () => {
  it('renders frequency options', () => {
    render(<StepWrapper><ScheduleStep /></StepWrapper>);
    expect(screen.getByTestId('frequency-options')).toBeInTheDocument();
    expect(screen.getByTestId('freq-daily')).toBeInTheDocument();
  });

  it('shows time picker for daily frequency', () => {
    render(<StepWrapper><ScheduleStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('freq-daily'));
    expect(screen.getByTestId('time-input')).toBeInTheDocument();
  });

  it('does not show time picker for manual frequency', () => {
    render(<StepWrapper><ScheduleStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('freq-manual'));
    // Should not show time input (goes to next step instead)
    expect(screen.queryByTestId('time-input')).not.toBeInTheDocument();
  });

  it('renders back button', () => {
    render(<StepWrapper><ScheduleStep /></StepWrapper>);
    expect(screen.getByTestId('freq-back')).toBeInTheDocument();
  });

  it('allows going back from time picker', () => {
    render(<StepWrapper><ScheduleStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('freq-daily'));
    expect(screen.getByTestId('time-input')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('time-back'));
    expect(screen.queryByTestId('time-input')).not.toBeInTheDocument();
  });
});

// ─── FolderStep ──────────────────────────────────────────────────────

describe('FolderStep', () => {
  it('renders folder options', () => {
    render(<StepWrapper><FolderStep /></StepWrapper>);
    expect(screen.getByTestId('folder-options')).toBeInTheDocument();
    expect(screen.getByTestId('folder-current')).toBeInTheDocument();
    expect(screen.getByTestId('folder-custom')).toBeInTheDocument();
  });

  it('shows custom path input when choosing custom folder', () => {
    render(<StepWrapper><FolderStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('folder-custom'));
    expect(screen.getByTestId('custom-folder-input')).toBeInTheDocument();
  });

  it('shows error for empty custom path', () => {
    render(<StepWrapper><FolderStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('folder-custom'));
    fireEvent.click(screen.getByTestId('folder-custom-submit'));
    expect(screen.getByTestId('folder-error')).toHaveTextContent('Path cannot be empty');
  });

  it('shows error for invalid path with ..', () => {
    render(<StepWrapper><FolderStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('folder-custom'));
    fireEvent.change(screen.getByTestId('custom-folder-input'), { target: { value: '../etc' } });
    fireEvent.click(screen.getByTestId('folder-custom-submit'));
    expect(screen.getByTestId('folder-error')).toHaveTextContent('Invalid path');
  });

  it('allows going back from custom path input', () => {
    render(<StepWrapper><FolderStep /></StepWrapper>);
    fireEvent.click(screen.getByTestId('folder-custom'));
    fireEvent.click(screen.getByTestId('folder-custom-back'));
    expect(screen.queryByTestId('custom-folder-input')).not.toBeInTheDocument();
  });
});

// ─── TaskConfirmStep ─────────────────────────────────────────────────

describe('TaskConfirmStep', () => {
  it('renders summary section', () => {
    render(<StepWrapper><TaskConfirmStep /></StepWrapper>);
    expect(screen.getByTestId('confirm-summary')).toBeInTheDocument();
  });

  it('shows default values for empty data', () => {
    render(<StepWrapper><TaskConfirmStep /></StepWrapper>);
    expect(screen.getByText('Name:')).toBeInTheDocument();
    // Multiple fields show '—' for empty data
    const summary = screen.getByTestId('confirm-summary');
    expect(summary.textContent).toContain('—');
  });

  it('renders confirm and back buttons', () => {
    render(<StepWrapper><TaskConfirmStep /></StepWrapper>);
    expect(screen.getByTestId('confirm-back')).toBeInTheDocument();
    expect(screen.getByTestId('confirm-submit')).toBeInTheDocument();
  });

  it('displays wizard data in summary', () => {
    render(
      <StepWrapper initialData={{ name: 'test-task', description: 'A test', model: 'claude-opus' }}>
        <TaskConfirmStep />
      </StepWrapper>,
    );
    expect(screen.getByText('test-task')).toBeInTheDocument();
    expect(screen.getByText('A test')).toBeInTheDocument();
    expect(screen.getByText('claude-opus')).toBeInTheDocument();
  });

  it('truncates long prompts', () => {
    const longPrompt = 'a'.repeat(100);
    render(
      <StepWrapper initialData={{ prompt: longPrompt }}>
        <TaskConfirmStep />
      </StepWrapper>,
    );
    expect(screen.getByText(`${'a'.repeat(57)}...`)).toBeInTheDocument();
  });
});
