import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { LspRecommendationMenu } from './LspRecommendationMenu';

afterEach(() => {
  cleanup();
});

describe('LspRecommendationMenu', () => {
  const defaultProps = {
    pluginName: 'TypeScript LSP',
    pluginDescription: 'TypeScript language server',
    fileExtension: '.ts',
    onResponse: vi.fn(),
  };

  it('renders LSP recommendation', () => {
    render(<LspRecommendationMenu {...defaultProps} />);
    expect(screen.getByTestId('lsp-recommendation-menu')).toBeInTheDocument();
    expect(screen.getByText('TypeScript LSP')).toBeInTheDocument();
  });

  it('shows file extension', () => {
    render(<LspRecommendationMenu {...defaultProps} />);
    expect(screen.getByText('.ts')).toBeInTheDocument();
  });

  it('calls onResponse with yes', () => {
    const onResponse = vi.fn();
    render(<LspRecommendationMenu {...defaultProps} onResponse={onResponse} />);
    fireEvent.click(screen.getByTestId('lsp-recommendation-yes'));
    expect(onResponse).toHaveBeenCalledWith('yes');
  });

  it('calls onResponse with no', () => {
    const onResponse = vi.fn();
    render(<LspRecommendationMenu {...defaultProps} onResponse={onResponse} />);
    fireEvent.click(screen.getByTestId('lsp-recommendation-no'));
    expect(onResponse).toHaveBeenCalledWith('no');
  });

  it('calls onResponse with never', () => {
    const onResponse = vi.fn();
    render(<LspRecommendationMenu {...defaultProps} onResponse={onResponse} />);
    fireEvent.click(screen.getByTestId('lsp-recommendation-never'));
    expect(onResponse).toHaveBeenCalledWith('never');
  });

  it('calls onResponse with disable', () => {
    const onResponse = vi.fn();
    render(<LspRecommendationMenu {...defaultProps} onResponse={onResponse} />);
    fireEvent.click(screen.getByTestId('lsp-recommendation-disable'));
    expect(onResponse).toHaveBeenCalledWith('disable');
  });
});
