import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { GenerateStep } from './GenerateStep';

afterEach(() => {
  cleanup();
});

describe('GenerateStep', () => {
  it('renders prompt', () => {
    render(<GenerateStep generating={false} prompt="Write code" />);
    expect(screen.getByTestId('generate-step')).toBeInTheDocument();
    expect(screen.getByText('Write code')).toBeInTheDocument();
  });

  it('shows generate button', () => {
    render(<GenerateStep generating={false} prompt="test" />);
    expect(screen.getByTestId('generate-step-button')).toBeInTheDocument();
  });

  it('shows loading state', () => {
    render(<GenerateStep generating={true} prompt="test" />);
    expect(screen.getByTestId('generate-step-loading')).toBeInTheDocument();
  });

  it('shows result', () => {
    render(<GenerateStep generating={false} prompt="test" result="Generated code" />);
    expect(screen.getByTestId('generate-step-result')).toHaveTextContent('Generated code');
  });

  it('calls onGenerate', () => {
    const onGenerate = vi.fn();
    render(<GenerateStep generating={false} prompt="test" onGenerate={onGenerate} />);
    fireEvent.click(screen.getByTestId('generate-step-button'));
    expect(onGenerate).toHaveBeenCalled();
  });
});
