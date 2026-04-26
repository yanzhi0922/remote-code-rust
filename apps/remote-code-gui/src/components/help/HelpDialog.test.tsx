import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { HelpDialog } from './HelpDialog';

describe('HelpDialog', () => {
  afterEach(() => { cleanup(); });

  it('renders help dialog with tabs', () => {
    const { getByTestId, getByText } = render(
      <HelpDialog onClose={() => {}} />,
    );
    expect(getByTestId('help-dialog')).toBeInTheDocument();
    expect(getByText('Help')).toBeInTheDocument();
    expect(getByTestId('help-tab-general')).toBeInTheDocument();
    expect(getByTestId('help-tab-commands')).toBeInTheDocument();
    expect(getByTestId('help-tab-shortcuts')).toBeInTheDocument();
  });

  it('shows default commands on commands tab', () => {
    const { getByText } = render(
      <HelpDialog onClose={() => {}} />,
    );
    // Default commands are shown on commands tab
    fireEvent.click(getByText('Commands').closest('button')!);
    expect(getByText('/help')).toBeInTheDocument();
    expect(getByText('/model')).toBeInTheDocument();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    const { getByTestId } = render(
      <HelpDialog onClose={onClose} />,
    );
    fireEvent.click(getByTestId('help-close-btn'));
    expect(onClose).toHaveBeenCalled();
  });

  it('shows custom commands when provided', () => {
    const { getByText } = render(
      <HelpDialog
        commands={[{ name: '/custom', description: 'Custom command' }]}
        onClose={() => {}}
      />,
    );
    fireEvent.click(getByText('Commands').closest('button')!);
    expect(getByText('/custom')).toBeInTheDocument();
  });
});
