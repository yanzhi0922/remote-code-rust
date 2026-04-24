import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Pane } from './Pane';

describe('Pane', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<Pane>内容</Pane>);
    expect(screen.getByTestId('pane')).toBeInTheDocument();
  });

  it('renders children content', () => {
    render(<Pane>面板内容</Pane>);
    expect(screen.getByText('面板内容')).toBeInTheDocument();
  });

  it('renders title when provided', () => {
    render(<Pane title="我的面板">内容</Pane>);
    expect(screen.getByTestId('pane-title')).toHaveTextContent('我的面板');
  });

  it('does not render header when no title', () => {
    render(<Pane>内容</Pane>);
    expect(screen.queryByTestId('pane-header')).not.toBeInTheDocument();
  });

  it('collapses content when collapsible and header is clicked', () => {
    render(<Pane title="可折叠" collapsible={true} defaultCollapsed={true}>隐藏内容</Pane>);
    expect(screen.queryByText('隐藏内容')).not.toBeInTheDocument();
  });

  it('expands content when collapsed header is clicked', () => {
    render(<Pane title="可折叠" collapsible={true} defaultCollapsed={true}>隐藏内容</Pane>);
    fireEvent.click(screen.getByTestId('pane-header'));
    expect(screen.getByText('隐藏内容')).toBeInTheDocument();
  });

  it('collapses content when clicking expanded header', () => {
    render(<Pane title="可折叠" collapsible={true}>显示内容</Pane>);
    fireEvent.click(screen.getByTestId('pane-header'));
    expect(screen.queryByTestId('pane-content')).not.toBeInTheDocument();
  });

  it('shows collapse icon when collapsible', () => {
    render(<Pane title="面板" collapsible={true}>内容</Pane>);
    expect(screen.getByTestId('pane-collapse-icon')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    render(<Pane className="custom">内容</Pane>);
    expect(screen.getByTestId('pane').className).toContain('custom');
  });
});
