import React, { useState, useEffect, useCallback } from 'react';

/**
 * Built-in Git panel for the sidebar, inspired by ZCode's integrated Git management.
 *
 * Features:
 * - Modified files list with status markers (M/U)
 * - Diff preview on file click
 * - Commit input with one-click commit
 * - Branch switcher
 * - Commit history
 */

interface GitFileStatus {
  path: string;
  status: 'M' | 'A' | 'D' | 'R' | 'C' | '?' | '!';
  isStaged: boolean;
}

interface GitBranch {
  name: string;
  isCurrent: boolean;
  isRemote: boolean;
}

interface CommitInfo {
  hash: string;
  shortHash: string;
  author: string;
  message: string;
  timestamp: number;
}

interface GitPanelProps {
  projectPath: string | null;
  className?: string;
}

type GitTab = 'changes' | 'history' | 'branches';

export const GitPanel: React.FC<GitPanelProps> = ({ projectPath, className = '' }) => {
  const [activeTab, setActiveTab] = useState<GitTab>('changes');
  const [files, setFiles] = useState<GitFileStatus[]>([]);
  const [branches, setBranchs] = useState<GitBranch[]>([]);
  const [commits, setCommits] = useState<CommitInfo[]>([]);
  const [commitMessage, setCommitMessage] = useState('');
  const [currentBranch, setCurrentBranch] = useState<string>('');
  const [loading, setLoading] = useState(false);

  const refreshStatus = useCallback(async () => {
    if (!projectPath) return;
    setLoading(true);
    try {
      // TODO: Call Tauri backend git_status command
      // For now, show placeholder
      setFiles([]);
      setCurrentBranch('main');
    } finally {
      setLoading(false);
    }
  }, [projectPath]);

  const refreshBranches = useCallback(async () => {
    if (!projectPath) return;
    try {
      // TODO: Call Tauri backend git_branches command
      setBranchs([{ name: 'main', isCurrent: true, isRemote: false }]);
    } finally {
      // done
    }
  }, [projectPath]);

  const refreshHistory = useCallback(async () => {
    if (!projectPath) return;
    try {
      // TODO: Call Tauri backend git_log command
      setCommits([]);
    } finally {
      // done
    }
  }, [projectPath]);

  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    if (activeTab === 'branches') refreshBranches();
    if (activeTab === 'history') refreshHistory();
  }, [activeTab, refreshBranches, refreshHistory]);

  const handleCommit = async () => {
    if (!commitMessage.trim() || !projectPath) return;
    try {
      // TODO: Call Tauri backend git_commit command
      setCommitMessage('');
      refreshStatus();
    } catch {
      // Error handled by UI state
    }
  };

  const handleStageFile = async (path: string) => {
    if (!projectPath) return;
    // TODO: Call Tauri backend git_stage command
    refreshStatus();
  };

  const handleSwitchBranch = async (name: string) => {
    if (!projectPath) return;
    // TODO: Call Tauri backend git_switch_branch command
    refreshStatus();
    refreshBranches();
  };

  const statusColor = (status: string) => {
    switch (status) {
      case 'M': return 'text-yellow-400';
      case 'A': return 'text-green-400';
      case 'D': return 'text-red-400';
      case '?': return 'text-gray-400';
      default: return 'text-gray-400';
    }
  };

  const tabs: { id: GitTab; label: string; icon: string }[] = [
    { id: 'changes', label: 'Changes', icon: '📝' },
    { id: 'history', label: 'History', icon: '📜' },
    { id: 'branches', label: 'Branches', icon: '🌿' },
  ];

  return (
    <div className={`flex flex-col h-full ${className}`}>
      {/* Tab bar */}
      <div className="flex border-b border-[var(--color-border)]">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`
              flex-1 px-3 py-2 text-xs font-medium transition-colors
              ${activeTab === tab.id
                ? 'text-[var(--color-text)] border-b-2 border-[var(--color-accent)]'
                : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
              }
            `}
          >
            <span className="mr-1">{tab.icon}</span>
            {tab.label}
            {tab.id === 'changes' && files.length > 0 && (
              <span className="ml-1 px-1.5 py-0.5 rounded-full bg-[var(--color-accent)] text-white text-[10px]">
                {files.length}
              </span>
            )}
          </button>
        ))}
      </div>

      {/* Branch indicator */}
      <div className="px-3 py-1.5 text-xs text-[var(--color-text-muted)] border-b border-[var(--color-border)]">
        🌿 {currentBranch || '—'}
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-y-auto">
        {activeTab === 'changes' && (
          <div className="p-2">
            {/* Commit input */}
            <div className="mb-3">
              <textarea
                value={commitMessage}
                onChange={(e) => setCommitMessage(e.target.value)}
                placeholder="Commit message..."
                className="w-full px-2 py-1.5 text-xs rounded border border-[var(--color-border)]
                  bg-[var(--color-input-bg)] text-[var(--color-text)]
                  placeholder:text-[var(--color-text-muted)]
                  focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]
                  resize-none"
                rows={2}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                    handleCommit();
                  }
                }}
              />
              <button
                onClick={handleCommit}
                disabled={!commitMessage.trim()}
                className="mt-1 w-full px-3 py-1.5 text-xs font-medium rounded
                  bg-[var(--color-accent)] text-white
                  disabled:opacity-50 disabled:cursor-not-allowed
                  hover:opacity-90 transition-opacity"
              >
                Commit (⌘+Enter)
              </button>
            </div>

            {/* File list */}
            {loading ? (
              <div className="text-xs text-[var(--color-text-muted)] text-center py-4">
                Loading...
              </div>
            ) : files.length === 0 ? (
              <div className="text-xs text-[var(--color-text-muted)] text-center py-4">
                No changes detected
              </div>
            ) : (
              <div className="space-y-0.5">
                {files.map((file) => (
                  <div
                    key={file.path}
                    className="flex items-center gap-2 px-2 py-1 rounded text-xs
                      hover:bg-[var(--color-surface-hover)] cursor-pointer group"
                    onClick={() => handleStageFile(file.path)}
                  >
                    <span className={`font-mono font-bold w-4 text-center ${statusColor(file.status)}`}>
                      {file.status}
                    </span>
                    <span className="flex-1 truncate text-[var(--color-text)]" title={file.path}>
                      {file.path}
                    </span>
                    {file.isStaged && (
                      <span className="text-[10px] text-green-400">✓</span>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === 'history' && (
          <div className="p-2">
            {commits.length === 0 ? (
              <div className="text-xs text-[var(--color-text-muted)] text-center py-4">
                No commit history
              </div>
            ) : (
              <div className="space-y-1">
                {commits.map((commit) => (
                  <div
                    key={commit.hash}
                    className="px-2 py-1.5 rounded text-xs hover:bg-[var(--color-surface-hover)]"
                  >
                    <div className="flex items-center gap-2">
                      <span className="font-mono text-[var(--color-accent)]">
                        {commit.shortHash}
                      </span>
                      <span className="text-[var(--color-text-muted)]">
                        {commit.author}
                      </span>
                    </div>
                    <div className="text-[var(--color-text)] mt-0.5 truncate">
                      {commit.message}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === 'branches' && (
          <div className="p-2">
            <div className="space-y-0.5">
              {branches.map((branch) => (
                <div
                  key={branch.name}
                  onClick={() => !branch.isCurrent && handleSwitchBranch(branch.name)}
                  className={`
                    flex items-center gap-2 px-2 py-1.5 rounded text-xs
                    ${branch.isCurrent
                      ? 'bg-[var(--color-accent)] bg-opacity-10 text-[var(--color-accent)]'
                      : 'hover:bg-[var(--color-surface-hover)] cursor-pointer text-[var(--color-text)]'
                    }
                  `}
                >
                  <span>{branch.isRemote ? '☁️' : '🌿'}</span>
                  <span className="flex-1 truncate">{branch.name}</span>
                  {branch.isCurrent && (
                    <span className="text-[10px]">✓ current</span>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default GitPanel;
