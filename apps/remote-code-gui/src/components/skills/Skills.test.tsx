import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { SkillsMenu } from './SkillsMenu';

afterEach(() => {
  cleanup();
});

const mockSkills = [
  { name: 'build', source: 'projectSettings', description: 'Build the project', tokenEstimate: 500 },
  { name: 'test', source: 'projectSettings', description: 'Run tests' },
  { name: 'deploy', source: 'userSettings', description: 'Deploy to prod' },
  { name: 'mcp:search', source: 'mcp', description: 'Search the web' },
];

describe('SkillsMenu', () => {
  it('renders skills grouped by source', () => {
    render(<SkillsMenu skills={mockSkills} onExit={vi.fn()} />);
    expect(screen.getByTestId('skills-menu')).toBeInTheDocument();
    expect(screen.getByText('ProjectSettings skills')).toBeInTheDocument();
    expect(screen.getByText('UserSettings skills')).toBeInTheDocument();
    expect(screen.getByText('MCP skills')).toBeInTheDocument();
  });

  it('shows empty state when no skills', () => {
    render(<SkillsMenu skills={[]} onExit={vi.fn()} />);
    expect(screen.getByText(/Create skills in/)).toBeInTheDocument();
  });

  it('calls onExit when close button clicked', () => {
    const onExit = vi.fn();
    render(<SkillsMenu skills={mockSkills} onExit={onExit} />);
    fireEvent.click(screen.getByTestId('skills-close-btn'));
    expect(onExit).toHaveBeenCalled();
  });

  it('calls onSelectSkill when skill clicked', () => {
    const onSelectSkill = vi.fn();
    render(<SkillsMenu skills={mockSkills} onExit={vi.fn()} onSelectSkill={onSelectSkill} />);
    fireEvent.click(screen.getByTestId('skill-item-build'));
    expect(onSelectSkill).toHaveBeenCalledWith('build');
  });

  it('shows token estimate', () => {
    render(<SkillsMenu skills={mockSkills} onExit={vi.fn()} />);
    expect(screen.getByText('~500 tokens')).toBeInTheDocument();
  });

  it('shows skill descriptions', () => {
    render(<SkillsMenu skills={mockSkills} onExit={vi.fn()} />);
    expect(screen.getByText('Build the project')).toBeInTheDocument();
  });
});
