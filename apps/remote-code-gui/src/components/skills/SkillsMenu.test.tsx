import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { SkillsMenu, type SkillInfo } from './SkillsMenu';

describe('SkillsMenu', () => {
  afterEach(() => { cleanup(); });

  it('renders empty state when no skills', () => {
    const { getByTestId, getByText } = render(
      <SkillsMenu skills={[]} onExit={() => {}} />,
    );
    expect(getByTestId('skills-menu')).toBeInTheDocument();
    expect(getByText(/Create skills/)).toBeInTheDocument();
  });

  it('renders skills grouped by source', () => {
    const skills: SkillInfo[] = [
      { name: 'skill-a', source: 'plugin', description: 'Plugin skill' },
      { name: 'skill-b', source: 'mcp', description: 'MCP skill' },
    ];
    const { getByText } = render(
      <SkillsMenu skills={skills} onExit={() => {}} />,
    );
    expect(getByText('Plugin skills')).toBeInTheDocument();
    expect(getByText('MCP skills')).toBeInTheDocument();
    expect(getByText('skill-a')).toBeInTheDocument();
    expect(getByText('skill-b')).toBeInTheDocument();
  });

  it('calls onExit when close button clicked', () => {
    const onExit = vi.fn();
    const { getByTestId } = render(
      <SkillsMenu skills={[]} onExit={onExit} />,
    );
    fireEvent.click(getByTestId('skills-close-btn'));
    expect(onExit).toHaveBeenCalled();
  });

  it('calls onSelectSkill when skill clicked', () => {
    const onSelectSkill = vi.fn();
    const skills: SkillInfo[] = [
      { name: 'my-skill', source: 'plugin' },
    ];
    const { getByTestId } = render(
      <SkillsMenu skills={skills} onExit={() => {}} onSelectSkill={onSelectSkill} />,
    );
    fireEvent.click(getByTestId('skill-item-my-skill'));
    expect(onSelectSkill).toHaveBeenCalledWith('my-skill');
  });
});
