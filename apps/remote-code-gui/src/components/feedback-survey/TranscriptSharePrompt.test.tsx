import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { TranscriptSharePrompt } from './TranscriptSharePrompt';

describe('TranscriptSharePrompt', () => {
  afterEach(() => { cleanup(); });

  it('renders transcript share prompt', () => {
    const { getByTestId } = render(
      <TranscriptSharePrompt onSelect={() => {}} inputValue="" setInputValue={() => {}} />,
    );
    expect(getByTestId('transcript-share-prompt')).toBeInTheDocument();
  });

  it('renders all response option buttons', () => {
    const { getByTestId } = render(
      <TranscriptSharePrompt onSelect={() => {}} inputValue="" setInputValue={() => {}} />,
    );
    expect(getByTestId('transcript-option-yes')).toBeInTheDocument();
    expect(getByTestId('transcript-option-no')).toBeInTheDocument();
    expect(getByTestId('transcript-option-dont_ask_again')).toBeInTheDocument();
  });

  it('calls onSelect with value when option clicked', () => {
    const onSelect = vi.fn();
    const { getByTestId } = render(
      <TranscriptSharePrompt onSelect={onSelect} inputValue="" setInputValue={() => {}} />,
    );
    fireEvent.click(getByTestId('transcript-option-yes'));
    expect(onSelect).toHaveBeenCalledWith('yes');
  });
});
