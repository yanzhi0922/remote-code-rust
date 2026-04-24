import React, { useState } from 'react';
import { X, ChevronLeft, Users, Shield, Eye } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface TeamMember {
  name: string;
  status: 'running' | 'idle' | 'stopped';
  permissionMode: string;
  role?: string;
}

export interface TeamInfo {
  name: string;
  members: TeamMember[];
}

type Props = {
  teams: TeamInfo[];
  onDone: () => void;
  onViewOutput?: (memberName: string) => void;
  onCycleMode?: (memberName: string) => void;
  onRemoveMember?: (memberName: string) => void;
};

const STATUS_COLORS: Record<string, string> = {
  running: 'bg-green-500',
  idle: 'bg-yellow-500',
  stopped: 'bg-gray-400',
};

const STATUS_LABELS: Record<string, string> = {
  running: 'Running',
  idle: 'Idle',
  stopped: 'Stopped',
};

export function TeamsDialog({
  teams,
  onDone,
  onViewOutput,
  onCycleMode,
  onRemoveMember,
}: Props): React.ReactElement {
  const [selectedTeam, setSelectedTeam] = useState<TeamInfo | null>(
    teams[0] ?? null,
  );
  const [selectedMember, setSelectedMember] = useState<TeamMember | null>(null);

  if (teams.length === 0) {
    return (
      <div
        data-testid="teams-dialog"
        className="rounded-lg border border-gray-200 bg-white p-4 shadow-lg dark:border-gray-700 dark:bg-gray-800"
      >
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Teams
          </h3>
          <button
            data-testid="teams-close-btn"
            aria-label="Close"
            onClick={onDone}
            className="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
          >
            <X className="h-5 w-5" />
          </button>
        </div>
        <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">
          No teams found.
        </p>
      </div>
    );
  }

  return (
    <div
      data-testid="teams-dialog"
      className="rounded-lg border border-gray-200 bg-white shadow-lg dark:border-gray-700 dark:bg-gray-800"
    >
      <div className="flex items-center justify-between border-b border-gray-200 px-4 py-3 dark:border-gray-700">
        <div className="flex items-center gap-2">
          {selectedMember && (
            <button
              data-testid="teams-back-btn"
              aria-label="Go back"
              onClick={() => setSelectedMember(null)}
              className="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
            >
              <ChevronLeft className="h-5 w-5" />
            </button>
          )}
          <Users className="h-5 w-5 text-gray-600 dark:text-gray-400" />
          <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            {selectedMember ? selectedMember.name : selectedTeam?.name ?? 'Teams'}
          </h3>
        </div>
        <button
          data-testid="teams-close-btn"
          aria-label="Close"
          onClick={onDone}
          className="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
        >
          <X className="h-5 w-5" />
        </button>
      </div>

      <div className="p-4">
        {!selectedMember ? (
          <div className="flex flex-col gap-1">
            {teams.length > 1 && (
              <div className="mb-2 flex gap-2">
                {teams.map((team) => (
                  <button
                    key={team.name}
                    data-testid={`teams-team-${team.name}`}
                    className={cn(
                      'rounded-md px-3 py-1 text-sm',
                      selectedTeam?.name === team.name
                        ? 'bg-cyan-100 text-cyan-700 dark:bg-cyan-900/30 dark:text-cyan-400'
                        : 'text-gray-600 hover:bg-gray-100 dark:text-gray-400 dark:hover:bg-gray-700',
                    )}
                    onClick={() => {
                      setSelectedTeam(team);
                    }}
                  >
                    {team.name}
                  </button>
                ))}
              </div>
            )}
            {selectedTeam?.members.map((member) => (
              <button
                key={member.name}
                data-testid={`teams-member-${member.name}`}
                className="flex items-center justify-between rounded-md px-3 py-2 text-left hover:bg-gray-50 dark:hover:bg-gray-700/50"
                onClick={() => setSelectedMember(member)}
              >
                <div className="flex items-center gap-2">
                  <span
                    className={cn(
                      'h-2.5 w-2.5 rounded-full',
                      STATUS_COLORS[member.status] ?? 'bg-gray-400',
                    )}
                  />
                  <span className="text-sm font-medium text-gray-900 dark:text-gray-100">
                    {member.name}
                  </span>
                  {member.role && (
                    <span className="text-xs text-gray-500 dark:text-gray-400">
                      ({member.role})
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-2">
                  <span className="text-xs text-gray-500 dark:text-gray-400">
                    {STATUS_LABELS[member.status]}
                  </span>
                  <span className="rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-600 dark:bg-gray-700 dark:text-gray-400">
                    {member.permissionMode}
                  </span>
                </div>
              </button>
            ))}
          </div>
        ) : (
          <div className="flex flex-col gap-3">
            <div className="flex items-center gap-2">
              <span
                className={cn(
                  'h-3 w-3 rounded-full',
                  STATUS_COLORS[selectedMember.status] ?? 'bg-gray-400',
                )}
              />
              <span className="text-sm text-gray-600 dark:text-gray-400">
                {STATUS_LABELS[selectedMember.status]}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <Shield className="h-4 w-4 text-gray-500" />
              <span className="text-sm text-gray-700 dark:text-gray-300">
                Permission mode: {selectedMember.permissionMode}
              </span>
            </div>
            <div className="flex gap-2">
              {onViewOutput && (
                <button
                  data-testid="teams-view-output-btn"
                  className="flex items-center gap-1 rounded-md bg-cyan-500 px-3 py-1.5 text-sm text-white hover:bg-cyan-600"
                  onClick={() => onViewOutput(selectedMember.name)}
                >
                  <Eye className="h-4 w-4" />
                  View Output
                </button>
              )}
              {onCycleMode && (
                <button
                  data-testid="teams-cycle-mode-btn"
                  className="rounded-md border border-gray-300 px-3 py-1.5 text-sm text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-700"
                  onClick={() => onCycleMode(selectedMember.name)}
                >
                  Cycle Mode
                </button>
              )}
              {onRemoveMember && (
                <button
                  data-testid="teams-remove-btn"
                  className="rounded-md border border-red-300 px-3 py-1.5 text-sm text-red-600 hover:bg-red-50 dark:border-red-600 dark:text-red-400 dark:hover:bg-red-900/20"
                  onClick={() => onRemoveMember(selectedMember.name)}
                >
                  Remove
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
