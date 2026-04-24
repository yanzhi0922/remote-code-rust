import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { OnboardingWizard } from './OnboardingWizard';

afterEach(() => {
  cleanup();
});

describe('OnboardingWizard', () => {
  it('renders nothing when visible is false', () => {
    render(<OnboardingWizard visible={false} onComplete={vi.fn()} />);
    expect(screen.queryByTestId('onboarding-wizard')).not.toBeInTheDocument();
  });

  it('renders wizard when visible is true', () => {
    render(<OnboardingWizard visible={true} onComplete={vi.fn()} />);
    expect(screen.getByTestId('onboarding-wizard')).toBeInTheDocument();
    expect(screen.getByText('欢迎使用 Remote Code')).toBeInTheDocument();
  });

  it('shows 4 step indicators', () => {
    render(<OnboardingWizard visible={true} onComplete={vi.fn()} />);
    expect(screen.getByTestId('step-dot-0')).toBeInTheDocument();
    expect(screen.getByTestId('step-dot-3')).toBeInTheDocument();
  });

  it('navigates to next step on next button click', () => {
    render(<OnboardingWizard visible={true} onComplete={vi.fn()} />);
    fireEvent.click(screen.getByTestId('next-button'));
    expect(screen.getByTestId('step-apikey')).toBeInTheDocument();
  });

  it('navigates to previous step on prev button click', () => {
    render(<OnboardingWizard visible={true} onComplete={vi.fn()} />);
    fireEvent.click(screen.getByTestId('next-button'));
    expect(screen.getByTestId('step-apikey')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('prev-button'));
    expect(screen.getByTestId('step-welcome')).toBeInTheDocument();
  });

  it('renders API key input as password type', () => {
    render(<OnboardingWizard visible={true} onComplete={vi.fn()} />);
    fireEvent.click(screen.getByTestId('next-button'));
    const input = screen.getByTestId('apikey-input') as HTMLInputElement;
    expect(input.type).toBe('password');
  });

  it('renders model select on step 3', () => {
    render(<OnboardingWizard visible={true} onComplete={vi.fn()} />);
    fireEvent.click(screen.getByTestId('next-button'));
    fireEvent.click(screen.getByTestId('next-button'));
    expect(screen.getByTestId('model-select')).toBeInTheDocument();
  });

  it('calls onComplete when skip button clicked', () => {
    const onComplete = vi.fn();
    render(<OnboardingWizard visible={true} onComplete={onComplete} />);
    fireEvent.click(screen.getByTestId('skip-button'));
    expect(onComplete).toHaveBeenCalled();
  });

  it('calls onComplete when finishing last step', () => {
    const onComplete = vi.fn();
    render(<OnboardingWizard visible={true} onComplete={onComplete} />);
    fireEvent.click(screen.getByTestId('next-button'));
    fireEvent.click(screen.getByTestId('next-button'));
    fireEvent.click(screen.getByTestId('next-button'));
    expect(screen.getByTestId('step-done')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('next-button'));
    expect(onComplete).toHaveBeenCalled();
  });

  it('highlights current step in indicator', () => {
    render(<OnboardingWizard visible={true} onComplete={vi.fn()} />);
    const step0 = screen.getByTestId('step-dot-0');
    expect(step0.className).toContain('bg-blue-600');
    const step1 = screen.getByTestId('step-dot-1');
    expect(step1.className).toContain('bg-slate-200');
  });
});
