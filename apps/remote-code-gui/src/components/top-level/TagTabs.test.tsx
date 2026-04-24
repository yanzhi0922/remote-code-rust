import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TagTabs } from './TagTabs';

afterEach(() => {
  cleanup();
});

describe('TagTabs', () => {
  const tabs = [
    { id: 'all', label: '全部', count: 10 },
    { id: 'active', label: '活跃' },
  ];

  it('renders tabs', () => {
    render(<TagTabs tabs={tabs} activeTab="all" onChange={() => {}} />);
    expect(screen.getByTestId('tag-tabs')).toBeInTheDocument();
    expect(screen.getByText('全部')).toBeInTheDocument();
  });

  it('shows count badge', () => {
    render(<TagTabs tabs={tabs} activeTab="all" onChange={() => {}} />);
    expect(screen.getByText('10')).toBeInTheDocument();
  });

  it('calls onChange', () => {
    const onChange = vi.fn();
    render(<TagTabs tabs={tabs} activeTab="all" onChange={onChange} />);
    fireEvent.click(screen.getByTestId('tag-tab-active'));
    expect(onChange).toHaveBeenCalledWith('active');
  });
});
