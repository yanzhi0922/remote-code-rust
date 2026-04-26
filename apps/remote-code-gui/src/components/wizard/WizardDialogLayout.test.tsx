import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { WizardContext, type WizardContextValue } from './WizardProvider';
import { WizardDialogLayout } from './WizardDialogLayout';

// Mock the WizardNavigationFooter since it's imported separately
vi.mock('./WizardNavigationFooter', () => ({
  WizardNavigationFooter: ({ instructions }: { instructions?: React.ReactNode }) => (
    <div data-testid="wizard-nav-footer">{instructions ?? 'default footer'}</div>
  ),
}));

const mockWizardContext: WizardContextValue = {
  currentStepIndex: 0,
  totalSteps: 3,
  title: 'Test Wizard',
  showStepCounter: true,
  wizardData: {},
  goNext: vi.fn(),
  goBack: vi.fn(),
  setWizardData: vi.fn(),
};

describe('WizardDialogLayout', () => {
  afterEach(() => { cleanup(); });

  it('renders layout with title and step counter', () => {
    const { getByTestId, getByText } = render(
      <WizardContext.Provider value={mockWizardContext}>
        <WizardDialogLayout>Content here</WizardDialogLayout>
      </WizardContext.Provider>,
    );
    expect(getByTestId('wizard-dialog-layout')).toBeInTheDocument();
    expect(getByText(/Test Wizard/)).toBeInTheDocument();
    expect(getByText(/\(1\/3\)/)).toBeInTheDocument();
  });

  it('renders children content', () => {
    const { getByText } = render(
      <WizardContext.Provider value={mockWizardContext}>
        <WizardDialogLayout>Child content</WizardDialogLayout>
      </WizardContext.Provider>,
    );
    expect(getByText('Child content')).toBeInTheDocument();
  });

  it('calls goBack when cancel button clicked', () => {
    const goBack = vi.fn();
    const { getByTestId } = render(
      <WizardContext.Provider value={{ ...mockWizardContext, goBack }}>
        <WizardDialogLayout>Content</WizardDialogLayout>
      </WizardContext.Provider>,
    );
    fireEvent.click(getByTestId('wizard-cancel-btn'));
    expect(goBack).toHaveBeenCalled();
  });
});
