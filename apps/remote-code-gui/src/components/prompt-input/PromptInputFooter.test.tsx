import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PromptInputFooter } from './PromptInputFooter';

describe('PromptInputFooter', () => {
  afterEach(cleanup);

  it('渲染并显示 data-testid', () => {
    render(<PromptInputFooter />);
    expect(screen.getByTestId('prompt-input-footer')).toBeInTheDocument();
  });

  it('显示模型名称', () => {
    render(<PromptInputFooter modelName="GPT-4" />);
    expect(screen.getByText('GPT-4')).toBeInTheDocument();
  });

  it('显示 token 用量', () => {
    render(<PromptInputFooter tokenCount={1000} maxTokens={8000} />);
    expect(screen.getByText('1,000 / 8,000')).toBeInTheDocument();
  });

  it('仅显示 token 数量（无 maxTokens）', () => {
    render(<PromptInputFooter tokenCount={500} />);
    expect(screen.getByText('500 tokens')).toBeInTheDocument();
  });

  it('isLoading 时显示 spinner', () => {
    render(<PromptInputFooter isLoading />);
    expect(screen.getByTestId('footer-spinner')).toBeInTheDocument();
  });

  it('isLoading=false 时不显示 spinner', () => {
    render(<PromptInputFooter isLoading={false} />);
    expect(screen.queryByTestId('footer-spinner')).not.toBeInTheDocument();
  });

  it('显示权限模式', () => {
    render(<PromptInputFooter permissionMode="auto" />);
    expect(screen.getByText('auto')).toBeInTheDocument();
  });

  it('无 token 信息时不显示 token 区域', () => {
    render(<PromptInputFooter />);
    expect(screen.queryByText(/tokens/)).not.toBeInTheDocument();
  });
});
