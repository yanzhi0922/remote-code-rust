import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { WizardProvider, useWizard } from './WizardProvider';
import { WizardDialogLayout } from './WizardDialogLayout';
import { WizardNavigationFooter } from './WizardNavigationFooter';

afterEach(() => {
  cleanup();
});

function TestWizardContent() {
  const { currentStepIndex, goNext, goBack, wizardData, setWizardData } = useWizard();
  return (
    <div>
      <span data-testid="wizard-step">{currentStepIndex}</span>
      <button data-testid="wizard-next" onClick={goNext}>Next</button>
      <button data-testid="wizard-back" onClick={goBack}>Back</button>
      <input
        data-testid="wizard-input"
        aria-label="Name"
        value={(wizardData as Record<string, string>).name ?? ''}
        onChange={(e) => setWizardData({ ...wizardData, name: e.target.value })}
      />
    </div>
  );
}

describe('WizardProvider', () => {
  it('provides wizard context', () => {
    render(
      <WizardProvider steps={[<div key="1" />]} onComplete={vi.fn()}>
        <TestWizardContent />
      </WizardProvider>,
    );
    expect(screen.getByTestId('wizard-step')).toHaveTextContent('0');
  });

  it('navigates to next step', () => {
    render(
      <WizardProvider steps={[<div key="1" />, <div key="2" />]} onComplete={vi.fn()}>
        <TestWizardContent />
      </WizardProvider>,
    );
    fireEvent.click(screen.getByTestId('wizard-next'));
    expect(screen.getByTestId('wizard-step')).toHaveTextContent('1');
  });

  it('calls onComplete on last step next', () => {
    const onComplete = vi.fn();
    render(
      <WizardProvider steps={[<div key="1" />]} onComplete={onComplete}>
        <TestWizardContent />
      </WizardProvider>,
    );
    fireEvent.click(screen.getByTestId('wizard-next'));
    expect(onComplete).toHaveBeenCalled();
  });

  it('calls onCancel on first step back', () => {
    const onCancel = vi.fn();
    render(
      <WizardProvider steps={[<div key="1" />]} onComplete={vi.fn()} onCancel={onCancel}>
        <TestWizardContent />
      </WizardProvider>,
    );
    fireEvent.click(screen.getByTestId('wizard-back'));
    expect(onCancel).toHaveBeenCalled();
  });

  it('manages wizard data', () => {
    render(
      <WizardProvider steps={[<div key="1" />]} onComplete={vi.fn()}>
        <TestWizardContent />
      </WizardProvider>,
    );
    fireEvent.change(screen.getByTestId('wizard-input'), { target: { value: 'test' } });
    expect(screen.getByTestId('wizard-input')).toHaveValue('test');
  });

  it('throws when useWizard is used outside provider', () => {
    // Suppress console.error for expected error
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => render(<TestWizardContent />)).toThrow(
      'useWizard must be used within a WizardProvider',
    );
    spy.mockRestore();
  });
});

describe('WizardDialogLayout', () => {
  it('renders with title and step counter', () => {
    render(
      <WizardProvider
        steps={[<div key="1" />, <div key="2" />]}
        onComplete={vi.fn()}
        title="Test Wizard"
      >
        <WizardDialogLayout>Step content</WizardDialogLayout>
      </WizardProvider>,
    );
    expect(screen.getByTestId('wizard-dialog-layout')).toBeInTheDocument();
    expect(screen.getByText('Test Wizard (1/2)')).toBeInTheDocument();
  });

  it('renders subtitle', () => {
    render(
      <WizardProvider steps={[<div key="1" />]} onComplete={vi.fn()}>
        <WizardDialogLayout subtitle="A test wizard">Content</WizardDialogLayout>
      </WizardProvider>,
    );
    expect(screen.getByText('A test wizard')).toBeInTheDocument();
  });

  it('calls goBack when cancel clicked', () => {
    const onCancel = vi.fn();
    render(
      <WizardProvider steps={[<div key="1" />]} onComplete={vi.fn()} onCancel={onCancel}>
        <WizardDialogLayout>Content</WizardDialogLayout>
      </WizardProvider>,
    );
    fireEvent.click(screen.getByTestId('wizard-cancel-btn'));
    expect(onCancel).toHaveBeenCalled();
  });

  it('renders navigation footer', () => {
    render(
      <WizardProvider steps={[<div key="1" />]} onComplete={vi.fn()}>
        <WizardDialogLayout>Content</WizardDialogLayout>
      </WizardProvider>,
    );
    expect(screen.getByTestId('wizard-navigation-footer')).toBeInTheDocument();
  });
});

describe('WizardNavigationFooter', () => {
  it('renders default instructions', () => {
    render(<WizardNavigationFooter />);
    expect(screen.getByTestId('wizard-navigation-footer')).toBeInTheDocument();
    expect(screen.getByText(/navigate/)).toBeInTheDocument();
  });

  it('renders custom instructions', () => {
    render(<WizardNavigationFooter instructions={<span>Custom footer</span>} />);
    expect(screen.getByText('Custom footer')).toBeInTheDocument();
  });
});
