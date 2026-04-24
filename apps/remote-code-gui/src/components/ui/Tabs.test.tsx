import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Tabs } from './Tabs';

afterEach(() => {
  cleanup();
});

const sampleTabs = [
  { key: 'tab1', label: 'Tab One' },
  { key: 'tab2', label: 'Tab Two' },
  { key: 'tab3', label: 'Tab Three' },
];

describe('Tabs', () => {
  it('renders all tab labels', () => {
    render(<Tabs tabs={sampleTabs} activeKey="tab1" onChange={vi.fn()} />);
    expect(screen.getByText('Tab One')).toBeInTheDocument();
    expect(screen.getByText('Tab Two')).toBeInTheDocument();
    expect(screen.getByText('Tab Three')).toBeInTheDocument();
  });

  it('highlights active tab', () => {
    render(<Tabs tabs={sampleTabs} activeKey="tab2" onChange={vi.fn()} />);
    const activeTab = screen.getByTestId('tab-tab2');
    expect(activeTab.className).toContain('border-slate-800');
    expect(activeTab.className).toContain('text-slate-900');
  });

  it('calls onChange when tab is clicked', () => {
    const onChange = vi.fn();
    render(<Tabs tabs={sampleTabs} activeKey="tab1" onChange={onChange} />);
    fireEvent.click(screen.getByTestId('tab-tab2'));
    expect(onChange).toHaveBeenCalledWith('tab2');
  });

  it('renders tab icons', () => {
    const tabsWithIcons = [
      { key: 'a', label: 'A', icon: <span data-testid="icon-a">🔥</span> },
      { key: 'b', label: 'B' },
    ];
    render(<Tabs tabs={tabsWithIcons} activeKey="a" onChange={vi.fn()} />);
    expect(screen.getByTestId('icon-a')).toBeInTheDocument();
  });

  it('navigates with ArrowRight key', () => {
    const onChange = vi.fn();
    render(<Tabs tabs={sampleTabs} activeKey="tab1" onChange={onChange} />);
    fireEvent.keyDown(screen.getByTestId('tab-tab1'), { key: 'ArrowRight' });
    expect(onChange).toHaveBeenCalledWith('tab2');
  });

  it('navigates with ArrowLeft key and wraps', () => {
    const onChange = vi.fn();
    render(<Tabs tabs={sampleTabs} activeKey="tab1" onChange={onChange} />);
    fireEvent.keyDown(screen.getByTestId('tab-tab1'), { key: 'ArrowLeft' });
    expect(onChange).toHaveBeenCalledWith('tab3');
  });

  it('wraps around with ArrowRight at last tab', () => {
    const onChange = vi.fn();
    render(<Tabs tabs={sampleTabs} activeKey="tab3" onChange={onChange} />);
    fireEvent.keyDown(screen.getByTestId('tab-tab3'), { key: 'ArrowRight' });
    expect(onChange).toHaveBeenCalledWith('tab1');
  });

  it('has tablist role', () => {
    render(<Tabs tabs={sampleTabs} activeKey="tab1" onChange={vi.fn()} />);
    expect(screen.getByTestId('tabs')).toHaveAttribute('role', 'tablist');
  });

  it('sets aria-selected on active tab', () => {
    render(<Tabs tabs={sampleTabs} activeKey="tab1" onChange={vi.fn()} />);
    expect(screen.getByTestId('tab-tab1')).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByTestId('tab-tab2')).toHaveAttribute('aria-selected', 'false');
  });

  it('merges custom className', () => {
    render(
      <Tabs
        tabs={sampleTabs}
        activeKey="tab1"
        onChange={vi.fn()}
        className="my-tabs"
      />,
    );
    expect(screen.getByTestId('tabs').className).toContain('my-tabs');
  });
});
