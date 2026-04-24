import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { IdeOnboardingDialog } from './IdeOnboardingDialog';

afterEach(() => {
  cleanup();
});

describe('IdeOnboardingDialog', () => {
  it('renders with data-testid', () => {
    render(<IdeOnboardingDialog onDone={vi.fn()} />);
    expect(screen.getByTestId('ide-onboarding-dialog')).toBeInTheDocument();
  });

  it('shows default IDE name', () => {
    render(<IdeOnboardingDialog onDone={vi.fn()} />);
    expect(screen.getByText(/VS Code/)).toBeInTheDocument();
  });

  it('shows custom IDE name', () => {
    render(<IdeOnboardingDialog onDone={vi.fn()} ideName="IntelliJ" />);
    expect(screen.getByText(/IntelliJ/)).toBeInTheDocument();
  });

  it('shows installed version', () => {
    render(<IdeOnboardingDialog onDone={vi.fn()} installedVersion="1.2.3" />);
    expect(screen.getByText(/installed extension v1\.2\.3/)).toBeInTheDocument();
  });

  it('calls onDone when Get Started is clicked', () => {
    const onDone = vi.fn();
    render(<IdeOnboardingDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('ide-onboarding-done'));
    expect(onDone).toHaveBeenCalledOnce();
  });

  it('calls onDone when close is clicked', () => {
    const onDone = vi.fn();
    render(<IdeOnboardingDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('ide-onboarding-close'));
    expect(onDone).toHaveBeenCalledOnce();
  });
});
