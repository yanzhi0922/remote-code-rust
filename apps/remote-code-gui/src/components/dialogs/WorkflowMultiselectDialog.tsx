import { type ReactNode, useState } from 'react';
import { GitBranch, X, ExternalLink, AlertCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

type Workflow = 'claude' | 'claude-review';

interface WorkflowOption {
  value: Workflow;
  label: string;
}

const WORKFLOWS: WorkflowOption[] = [
  { value: 'claude', label: '@Claude Code - Tag @claude in issues and PR comments' },
  { value: 'claude-review', label: 'Claude Code Review - Automated code review on new PRs' },
];

interface Props {
  onSubmit: (selectedWorkflows: Workflow[]) => void;
  defaultSelections?: Workflow[];
}

export function WorkflowMultiselectDialog({
  onSubmit,
  defaultSelections = [],
}: Props): ReactNode {
  const [selected, setSelected] = useState<Set<Workflow>>(new Set(defaultSelections));
  const [showError, setShowError] = useState(false);

  const toggleWorkflow = (workflow: Workflow) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(workflow)) {
        next.delete(workflow);
      } else {
        next.add(workflow);
      }
      return next;
    });
    setShowError(false);
  };

  const handleSubmit = () => {
    const selectedArray = Array.from(selected);
    if (selectedArray.length === 0) {
      setShowError(true);
      return;
    }
    onSubmit(selectedArray);
  };

  const handleCancel = () => {
    setShowError(true);
  };

  return (
    <div
      data-testid="workflow-multiselect-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <GitBranch className="h-5 w-5 text-green-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Select Workflows
            </h3>
          </div>
          <button
            data-testid="workflow-multiselect-close"
            onClick={handleCancel}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="mt-3 space-y-2">
          {WORKFLOWS.map((workflow) => {
            const isSelected = selected.has(workflow.value);
            return (
              <button
                key={workflow.value}
                data-testid={`workflow-multiselect-${workflow.value}`}
                onClick={() => toggleWorkflow(workflow.value)}
                className={cn(
                  'flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm',
                  isSelected
                    ? 'border-2 border-green-500 bg-green-50 dark:bg-green-950'
                    : 'border border-gray-200 bg-gray-50 hover:bg-gray-100 dark:border-gray-600 dark:bg-gray-700 dark:hover:bg-gray-600',
                )}
              >
                <span className="flex-1 text-gray-900 dark:text-gray-100">{workflow.label}</span>
                {isSelected && <span className="text-green-500">✓</span>}
              </button>
            );
          })}
        </div>

        {showError && (
          <div className="mt-2 flex items-center gap-1">
            <AlertCircle className="h-4 w-4 text-red-500" />
            <p className="text-sm text-red-600 dark:text-red-400">
              Please select at least one workflow.
            </p>
          </div>
        )}

        <div className="mt-3">
          <a
            href="https://github.com/anthropics/claude-code-action/blob/main/examples/"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400"
          >
            More workflow examples (issue triage, CI fixes, etc.){' '}
            <ExternalLink className="h-3 w-3" />
          </a>
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <button
            data-testid="workflow-multiselect-cancel"
            onClick={handleCancel}
            className="rounded bg-gray-100 px-4 py-2 text-sm text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
          >
            Cancel
          </button>
          <button
            data-testid="workflow-multiselect-confirm"
            onClick={handleSubmit}
            className="rounded bg-green-600 px-4 py-2 text-sm text-white hover:bg-green-700"
          >
            Install ({selected.size})
          </button>
        </div>
      </div>
    </div>
  );
}
