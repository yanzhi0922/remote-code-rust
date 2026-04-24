import { type ReactNode } from 'react';
import { WizardProvider, useWizard } from '../wizard';
import type { ScheduledTaskWizardData } from './types';
import { NameStep } from './steps/NameStep';
import { TaskDescriptionStep } from './steps/TaskDescriptionStep';
import { TaskPromptStep } from './steps/TaskPromptStep';
import { TaskModelStep } from './steps/TaskModelStep';
import { PermissionStep } from './steps/PermissionStep';
import { FolderStep } from './steps/FolderStep';
import { ScheduleStep } from './steps/ScheduleStep';
import { TaskConfirmStep } from './steps/TaskConfirmStep';

type Props = {
  mode: 'create' | 'edit';
  initialData?: Partial<ScheduledTaskWizardData>;
  onComplete: (data: ScheduledTaskWizardData) => void;
  onCancel: () => void;
};

const STEP_COUNT = 8;

function ScheduledTaskWizardInner(): ReactNode {
  const { currentStepIndex } = useWizard<ScheduledTaskWizardData>();

  const steps: ReactNode[] = [
    <NameStep key="name" />,
    <TaskDescriptionStep key="desc" />,
    <TaskPromptStep key="prompt" />,
    <TaskModelStep key="model" />,
    <PermissionStep key="perm" />,
    <FolderStep key="folder" />,
    <ScheduleStep key="schedule" />,
    <TaskConfirmStep key="confirm" />,
  ];

  return <div data-testid="scheduled-task-wizard-steps">{steps[currentStepIndex] ?? null}</div>;
}

export function ScheduledTaskWizard({
  mode,
  initialData = {},
  onComplete,
  onCancel,
}: Props): ReactNode {
  const title = mode === 'create' ? 'New scheduled task' : 'Edit scheduled task';
  const placeholderSteps = Array.from({ length: STEP_COUNT }, (_, i) => (
    <div key={i} />
  ));

  return (
    <div data-testid="scheduled-task-wizard">
      <WizardProvider
        steps={placeholderSteps}
        initialData={initialData as ScheduledTaskWizardData}
        onComplete={onComplete}
        onCancel={onCancel}
        title={title}
        showStepCounter={true}
      >
        <ScheduledTaskWizardInner />
      </WizardProvider>
    </div>
  );
}
