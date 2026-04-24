import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AutoModeOptIn } from './AutoModeOptIn';

describe('AutoModeOptIn', () => {
  afterEach(cleanup);

  it('renders nothing when not visible', () => {
    const { container } = render(
      <AutoModeOptIn visible={false} onConfirm={vi.fn()} onCancel={vi.fn()} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('renders dialog when visible', () => {
    render(<AutoModeOptIn visible={true} onConfirm={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByTestId('auto-mode-dialog')).toBeInTheDocument();
    expect(screen.getByText('启用自动模式')).toBeInTheDocument();
  });

  it('shows warning message', () => {
    render(<AutoModeOptIn visible={true} onConfirm={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByText(/自动模式将跳过权限确认/)).toBeInTheDocument();
  });

  it('shows three risk tiers', () => {
    render(<AutoModeOptIn visible={true} onConfirm={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByText('保守')).toBeInTheDocument();
    expect(screen.getByText('适中')).toBeInTheDocument();
    expect(screen.getByText('激进')).toBeInTheDocument();
  });

  it('disables confirm button until correct text is entered', () => {
    render(<AutoModeOptIn visible={true} onConfirm={vi.fn()} onCancel={vi.fn()} />);
    const confirmBtn = screen.getByText('确认启用');
    expect(confirmBtn).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText('AUTO MODE'), {
      target: { value: 'wrong text' },
    });
    expect(confirmBtn).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText('AUTO MODE'), {
      target: { value: 'AUTO MODE' },
    });
    expect(confirmBtn).not.toBeDisabled();
  });

  it('calls onConfirm with rules when confirmed', () => {
    const onConfirm = vi.fn();
    render(<AutoModeOptIn visible={true} onConfirm={onConfirm} onCancel={vi.fn()} />);

    fireEvent.change(screen.getByPlaceholderText('AUTO MODE'), {
      target: { value: 'AUTO MODE' },
    });
    fireEvent.click(screen.getByText('确认启用'));

    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onConfirm).toHaveBeenCalledWith(
      expect.arrayContaining(['ls', 'cat', 'grep']),
    );
  });

  it('calls onCancel when cancel is clicked', () => {
    const onCancel = vi.fn();
    render(<AutoModeOptIn visible={true} onConfirm={vi.fn()} onCancel={onCancel} />);
    fireEvent.click(screen.getByText('取消'));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('changes tier selection on click', () => {
    render(<AutoModeOptIn visible={true} onConfirm={vi.fn()} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByText('激进'));
    expect(screen.getByText('docker *')).toBeInTheDocument();
  });
});
