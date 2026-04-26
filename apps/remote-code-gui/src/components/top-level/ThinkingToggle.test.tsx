import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { ThinkingToggle } from './ThinkingToggle';

describe('ThinkingToggle', () => {
  afterEach(() => { cleanup(); });

  it('renders enable and disable buttons', () => {
    const { getByTestId } = render(
      <ThinkingToggle currentValue={false} onSelect={() => {}} />,
    );
    expect(getByTestId('thinking-toggle')).toBeInTheDocument();
    expect(getByTestId('thinking-enable')).toBeInTheDocument();
    expect(getByTestId('thinking-disable')).toBeInTheDocument();
  });

  it('calls onSelect(true) when enable clicked', () => {
    const onSelect = vi.fn();
    const { getByTestId } = render(
      <ThinkingToggle currentValue={false} onSelect={onSelect} />,
    );
    fireEvent.click(getByTestId('thinking-enable'));
    expect(onSelect).toHaveBeenCalledWith(true);
  });

  it('calls onSelect(false) when disable clicked', () => {
    const onSelect = vi.fn();
    const { getByTestId } = render(
      <ThinkingToggle currentValue={true} onSelect={onSelect} />,
    );
    fireEvent.click(getByTestId('thinking-disable'));
    expect(onSelect).toHaveBeenCalledWith(false);
  });

  it('shows mid-conversation warning when isMidConversation is true', () => {
    const { getByText } = render(
      <ThinkingToggle currentValue={false} onSelect={() => {}} isMidConversation />,
    );
    expect(getByText(/Changing thinking mode mid-conversation/)).toBeInTheDocument();
  });

  it('renders cancel button when onCancel provided', () => {
    const { getByTestId } = render(
      <ThinkingToggle currentValue={false} onSelect={() => {}} onCancel={() => {}} />,
    );
    expect(getByTestId('thinking-cancel')).toBeInTheDocument();
  });
});
