import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { TeamsDialog } from './TeamsDialog';
import { TeamStatus } from './TeamStatus';

afterEach(() => {
  cleanup();
});

const mockTeams = [
  {
    name: 'team-alpha',
    members: [
      { name: 'agent-1', status: 'running' as const, permissionMode: 'plan' },
      { name: 'agent-2', status: 'idle' as const, permissionMode: 'auto-accept' },
    ],
  },
];

describe('TeamsDialog', () => {
  it('renders with team members', () => {
    render(<TeamsDialog teams={mockTeams} onDone={vi.fn()} />);
    expect(screen.getByTestId('teams-dialog')).toBeInTheDocument();
    expect(screen.getByTestId('teams-member-agent-1')).toBeInTheDocument();
    expect(screen.getByTestId('teams-member-agent-2')).toBeInTheDocument();
  });

  it('shows empty state when no teams', () => {
    render(<TeamsDialog teams={[]} onDone={vi.fn()} />);
    expect(screen.getByText('No teams found.')).toBeInTheDocument();
  });

  it('calls onDone when close button clicked', () => {
    const onDone = vi.fn();
    render(<TeamsDialog teams={mockTeams} onDone={onDone} />);
    fireEvent.click(screen.getByTestId('teams-close-btn'));
    expect(onDone).toHaveBeenCalled();
  });

  it('shows member detail when member clicked', () => {
    render(<TeamsDialog teams={mockTeams} onDone={vi.fn()} />);
    fireEvent.click(screen.getByTestId('teams-member-agent-1'));
    expect(screen.getByText('agent-1')).toBeInTheDocument();
    expect(screen.getByText(/Permission mode: plan/)).toBeInTheDocument();
  });

  it('shows back button in member detail view', () => {
    render(<TeamsDialog teams={mockTeams} onDone={vi.fn()} />);
    fireEvent.click(screen.getByTestId('teams-member-agent-1'));
    expect(screen.getByTestId('teams-back-btn')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('teams-back-btn'));
    expect(screen.getByTestId('teams-member-agent-1')).toBeInTheDocument();
  });

  it('calls onViewOutput when view output clicked', () => {
    const onViewOutput = vi.fn();
    render(
      <TeamsDialog teams={mockTeams} onDone={vi.fn()} onViewOutput={onViewOutput} />,
    );
    fireEvent.click(screen.getByTestId('teams-member-agent-1'));
    fireEvent.click(screen.getByTestId('teams-view-output-btn'));
    expect(onViewOutput).toHaveBeenCalledWith('agent-1');
  });

  it('calls onCycleMode when cycle mode clicked', () => {
    const onCycleMode = vi.fn();
    render(
      <TeamsDialog teams={mockTeams} onDone={vi.fn()} onCycleMode={onCycleMode} />,
    );
    fireEvent.click(screen.getByTestId('teams-member-agent-1'));
    fireEvent.click(screen.getByTestId('teams-cycle-mode-btn'));
    expect(onCycleMode).toHaveBeenCalledWith('agent-1');
  });
});

describe('TeamStatus', () => {
  it('returns null when teammateCount is 0', () => {
    const { container } = render(
      <TeamStatus teamsSelected={false} showHint={false} teammateCount={0} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('renders teammate count', () => {
    render(<TeamStatus teamsSelected={false} showHint={false} teammateCount={3} />);
    expect(screen.getByTestId('team-status')).toBeInTheDocument();
    expect(screen.getByText('3 teammates')).toBeInTheDocument();
  });

  it('shows singular form for 1 teammate', () => {
    render(<TeamStatus teamsSelected={false} showHint={false} teammateCount={1} />);
    expect(screen.getByText('1 teammate')).toBeInTheDocument();
  });

  it('shows hint when teamsSelected and showHint are true', () => {
    render(<TeamStatus teamsSelected={true} showHint={true} teammateCount={2} />);
    expect(screen.getByText('· Enter to view')).toBeInTheDocument();
  });

  it('does not show hint when teamsSelected is false', () => {
    render(<TeamStatus teamsSelected={false} showHint={true} teammateCount={2} />);
    expect(screen.queryByText('· Enter to view')).not.toBeInTheDocument();
  });
});
