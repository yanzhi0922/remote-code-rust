import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { TrustDialog, type TrustWarning } from './TrustDialog';

describe('TrustDialog', () => {
  afterEach(() => { cleanup(); });

  it('renders trust dialog with accept and decline buttons', () => {
    const { getByTestId, getByText } = render(
      <TrustDialog onAccept={() => {}} onDecline={() => {}} />,
    );
    expect(getByTestId('trust-dialog')).toBeInTheDocument();
    expect(getByTestId('trust-accept-btn')).toBeInTheDocument();
    expect(getByTestId('trust-decline-btn')).toBeInTheDocument();
    expect(getByText('Trust This Project?')).toBeInTheDocument();
  });

  it('shows project name when provided', () => {
    const { getByText } = render(
      <TrustDialog projectName="my-project" onAccept={() => {}} onDecline={() => {}} />,
    );
    expect(getByText('my-project')).toBeInTheDocument();
  });

  it('renders warnings', () => {
    const warnings: TrustWarning[] = [
      { type: 'exec', label: 'Code Execution', description: 'May run scripts' },
    ];
    const { getByTestId, getByText } = render(
      <TrustDialog warnings={warnings} onAccept={() => {}} onDecline={() => {}} />,
    );
    expect(getByTestId('trust-warning-exec')).toBeInTheDocument();
    expect(getByText('Code Execution')).toBeInTheDocument();
  });

  it('calls onAccept and shows accepted state', () => {
    const onAccept = vi.fn();
    const { getByTestId } = render(
      <TrustDialog onAccept={onAccept} onDecline={() => {}} />,
    );
    fireEvent.click(getByTestId('trust-accept-btn'));
    expect(onAccept).toHaveBeenCalled();
    expect(getByTestId('trust-dialog-accepted')).toBeInTheDocument();
  });

  it('calls onDecline when decline clicked', () => {
    const onDecline = vi.fn();
    const { getByTestId } = render(
      <TrustDialog onAccept={() => {}} onDecline={onDecline} />,
    );
    fireEvent.click(getByTestId('trust-decline-btn'));
    expect(onDecline).toHaveBeenCalled();
  });
});
