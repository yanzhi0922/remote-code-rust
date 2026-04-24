import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { HelpV2 } from './HelpV2';

afterEach(() => {
  cleanup();
});

describe('HelpV2', () => {
  it('renders nothing when closed', () => {
    const { container } = render(<HelpV2 commands={[]} open={false} onClose={() => {}} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders dialog when open', () => {
    render(<HelpV2 commands={[]} open={true} onClose={() => {}} />);
    expect(screen.getByTestId('help-v2-dialog')).toBeInTheDocument();
  });

  it('shows default commands', () => {
    render(<HelpV2 open={true} onClose={() => {}} />);
    expect(screen.getByTestId('help-v2-command-help')).toBeInTheDocument();
    expect(screen.getByTestId('help-v2-command-clear')).toBeInTheDocument();
  });

  it('shows custom commands', () => {
    const commands = [
      { name: '/test', description: '测试命令', category: '测试' },
    ];
    render(<HelpV2 commands={commands} open={true} onClose={() => {}} />);
    expect(screen.getByTestId('help-v2-command-test')).toBeInTheDocument();
  });

  it('filters commands by search', () => {
    render(<HelpV2 open={true} onClose={() => {}} />);
    const search = screen.getByTestId('help-v2-search');
    fireEvent.change(search, { target: { value: 'help' } });
    expect(screen.getByTestId('help-v2-command-help')).toBeInTheDocument();
    expect(screen.queryByTestId('help-v2-command-clear')).not.toBeInTheDocument();
  });

  it('shows empty state when no matches', () => {
    render(<HelpV2 commands={[]} open={true} onClose={() => {}} />);
    const search = screen.getByTestId('help-v2-search');
    fireEvent.change(search, { target: { value: 'zzzzz' } });
    expect(screen.getByTestId('help-v2-empty')).toHaveTextContent('没有匹配的命令');
  });

  it('calls onClose when backdrop clicked', () => {
    let closed = false;
    render(<HelpV2 commands={[]} open={true} onClose={() => { closed = true; }} />);
    fireEvent.click(screen.getByTestId('help-v2-backdrop'));
    expect(closed).toBe(true);
  });

  it('calls onClose when close button clicked', () => {
    let closed = false;
    render(<HelpV2 commands={[]} open={true} onClose={() => { closed = true; }} />);
    fireEvent.click(screen.getByTestId('help-v2-close'));
    expect(closed).toBe(true);
  });
});
