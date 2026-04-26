import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { WizardProvider, useWizard } from './WizardProvider';

function StepContent({ label }: { label: string }) {
  const { goNext, goBack, currentStepIndex, totalSteps } = useWizard();
  return (
    <div>
      <span data-testid="step-label">{label}</span>
      <span data-testid="step-info">{currentStepIndex}/{totalSteps}</span>
      <button data-testid="go-next" onClick={goNext}>Next</button>
      <button data-testid="go-back" onClick={goBack}>Back</button>
    </div>
  );
}

describe('WizardProvider', () => {
  afterEach(() => { cleanup(); });

  it('renders first step initially', () => {
    const { getByTestId } = render(
      <WizardProvider steps={[<StepContent key="1" label="Step 1" />]} onComplete={() => {}}>
        <StepContent label="Step 1" />
      </WizardProvider>,
    );
    expect(getByTestId('step-label')).toHaveTextContent('Step 1');
  });

  it('advances to next step when goNext called', () => {
    const { getByTestId } = render(
      <WizardProvider
        steps={[<StepContent key="1" label="Step 1" />, <StepContent key="2" label="Step 2" />]}
        onComplete={() => {}}
      >
        <StepContent label="Step 1" />
      </WizardProvider>,
    );
    expect(getByTestId('step-info')).toHaveTextContent('0/2');
    fireEvent.click(getByTestId('go-next'));
    expect(getByTestId('step-info')).toHaveTextContent('1/2');
  });

  it('calls onComplete when goNext on last step', () => {
    const onComplete = vi.fn();
    const { getByTestId } = render(
      <WizardProvider steps={[<StepContent key="1" label="Step 1" />]} onComplete={onComplete}>
        <StepContent label="Step 1" />
      </WizardProvider>,
    );
    fireEvent.click(getByTestId('go-next'));
    expect(onComplete).toHaveBeenCalled();
  });

  it('calls onCancel when goBack on first step', () => {
    const onCancel = vi.fn();
    const { getByTestId } = render(
      <WizardProvider steps={[<StepContent key="1" label="Step 1" />]} onComplete={() => {}} onCancel={onCancel}>
        <StepContent label="Step 1" />
      </WizardProvider>,
    );
    fireEvent.click(getByTestId('go-back'));
    expect(onCancel).toHaveBeenCalled();
  });
});
