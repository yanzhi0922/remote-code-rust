import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ListItem } from './ListItem';
import { Bot } from 'lucide-react';

describe('ListItem', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<ListItem label="测试项" />);
    expect(screen.getByTestId('list-item')).toBeInTheDocument();
  });

  it('renders label text', () => {
    render(<ListItem label="我的项目" />);
    expect(screen.getByTestId('list-item-label')).toHaveTextContent('我的项目');
  });

  it('renders description when provided', () => {
    render(<ListItem label="项目" description="项目描述" />);
    expect(screen.getByTestId('list-item-description')).toHaveTextContent('项目描述');
  });

  it('does not render description element when not provided', () => {
    render(<ListItem label="项目" />);
    expect(screen.queryByTestId('list-item-description')).not.toBeInTheDocument();
  });

  it('shows selected state', () => {
    render(<ListItem label="项目" selected={true} />);
    expect(screen.getByTestId('list-item-selected')).toBeInTheDocument();
    expect(screen.getByTestId('list-item').className).toContain('border-blue-500');
  });

  it('does not show selected indicator when not selected', () => {
    render(<ListItem label="项目" selected={false} />);
    expect(screen.queryByTestId('list-item-selected')).not.toBeInTheDocument();
  });

  it('calls onClick when clicked', () => {
    const onClick = vi.fn();
    render(<ListItem label="项目" onClick={onClick} />);
    fireEvent.click(screen.getByTestId('list-item'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('renders icon when provided', () => {
    render(<ListItem label="项目" icon={<Bot data-testid="custom-icon" />} />);
    expect(screen.getByTestId('list-item-icon')).toBeInTheDocument();
    expect(screen.getByTestId('custom-icon')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    render(<ListItem label="项目" className="extra-cls" />);
    expect(screen.getByTestId('list-item').className).toContain('extra-cls');
  });
});
