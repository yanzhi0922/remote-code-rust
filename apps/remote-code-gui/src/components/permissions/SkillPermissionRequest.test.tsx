import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { SkillPermissionRequest } from './SkillPermissionRequest';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-skill-1',
    tool_name: 'Skill',
    tool_use_id: 'tool-skill-1',
    title: 'Skill Execution',
    description: 'Execute a skill',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('SkillPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(
      <SkillPermissionRequest
        request={makeRequest({ input: { skill_name: 'my-skill' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByTestId('skill-permission-request')).toBeInTheDocument();
  });

  it('displays the skill name', () => {
    render(
      <SkillPermissionRequest
        request={makeRequest({ input: { skill_name: 'deploy-to-azure' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('deploy-to-azure')).toBeInTheDocument();
  });

  it('shows skill name label', () => {
    render(
      <SkillPermissionRequest
        request={makeRequest({ input: { skill_name: 'test-skill' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('技能名称:')).toBeInTheDocument();
  });

  it('shows no skill name message when missing', () => {
    render(
      <SkillPermissionRequest
        request={makeRequest({ input: {} })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('无技能名称')).toBeInTheDocument();
  });

  it('calls onAllow when allow button is clicked', () => {
    const onAllow = vi.fn();
    render(
      <SkillPermissionRequest
        request={makeRequest({ input: { skill_name: 'my-skill' } })}
        onAllow={onAllow}
        onReject={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByText('允许执行'));
    expect(onAllow).toHaveBeenCalledTimes(1);
  });

  it('calls onReject when reject button is clicked', () => {
    const onReject = vi.fn();
    render(
      <SkillPermissionRequest
        request={makeRequest({ input: { skill_name: 'my-skill' } })}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });

  it('handles string input gracefully', () => {
    render(
      <SkillPermissionRequest
        request={makeRequest({ input: 'not-an-object' as unknown as Record<string, unknown> })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('无技能名称')).toBeInTheDocument();
  });
});
