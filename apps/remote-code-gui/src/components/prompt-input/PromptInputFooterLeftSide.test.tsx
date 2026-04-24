import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PromptInputFooterLeftSide } from './PromptInputFooterLeftSide';

describe('PromptInputFooterLeftSide', () => {
  afterEach(cleanup);

  it('渲染并显示 data-testid', () => {
    render(<PromptInputFooterLeftSide />);
    expect(screen.getByTestId('prompt-footer-left')).toBeInTheDocument();
  });

  it('显示模型名称徽章', () => {
    render(<PromptInputFooterLeftSide modelName="Claude" />);
    expect(screen.getByText('Claude')).toBeInTheDocument();
  });

  it('不传 modelName 时不显示模型徽章', () => {
    render(<PromptInputFooterLeftSide />);
    expect(screen.queryByText('Claude')).not.toBeInTheDocument();
  });

  it('显示权限模式标签', () => {
    render(<PromptInputFooterLeftSide permissionMode="yolo" />);
    expect(screen.getByText('yolo')).toBeInTheDocument();
  });

  it('不传 permissionMode 时不显示权限标签', () => {
    render(<PromptInputFooterLeftSide />);
    expect(screen.queryByText('yolo')).not.toBeInTheDocument();
  });

  it('同时显示模型和权限模式', () => {
    render(
      <PromptInputFooterLeftSide modelName="GPT-4" permissionMode="auto" />,
    );
    expect(screen.getByText('GPT-4')).toBeInTheDocument();
    expect(screen.getByText('auto')).toBeInTheDocument();
  });
});
