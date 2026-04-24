import { type ReactNode } from 'react';
import { Server, X, ShieldAlert } from 'lucide-react';
import { cn } from '../../lib/utils';

interface Props {
  serverName: string;
  onDone: (choice: 'yes_all' | 'yes' | 'no') => void;
}

export function McpServerApprovalDialog({ serverName, onDone }: Props): ReactNode {
  return (
    <div
      data-testid="mcp-server-approval-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Server className="h-5 w-5 text-blue-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              New MCP Server Found
            </h3>
          </div>
          <button
            data-testid="mcp-server-approval-close"
            onClick={() => onDone('no')}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <p className="mt-3 text-sm text-gray-600 dark:text-gray-400">
          New MCP server found in .mcp.json: <span className="font-semibold">{serverName}</span>
        </p>

        <div className="mt-2 flex items-start gap-1">
          <ShieldAlert className="mt-0.5 h-4 w-4 shrink-0 text-amber-500" />
          <p className="text-xs text-amber-600 dark:text-amber-400">
            Only use MCP servers from sources you trust. MCP servers can execute code and access your files.
          </p>
        </div>

        <div className="mt-4 flex flex-col gap-2">
          <button
            data-testid="mcp-server-approval-yes-all"
            onClick={() => onDone('yes_all')}
            className={cn(
              'rounded px-4 py-2 text-left text-sm',
              'bg-blue-50 text-blue-700 hover:bg-blue-100',
              'dark:bg-blue-950 dark:text-blue-300 dark:hover:bg-blue-900',
            )}
          >
            Use this and all future MCP servers in this project
          </button>
          <button
            data-testid="mcp-server-approval-yes"
            onClick={() => onDone('yes')}
            className={cn(
              'rounded px-4 py-2 text-left text-sm',
              'bg-gray-50 text-gray-700 hover:bg-gray-100',
              'dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600',
            )}
          >
            Use this MCP server
          </button>
          <button
            data-testid="mcp-server-approval-no"
            onClick={() => onDone('no')}
            className="rounded px-4 py-2 text-left text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400"
          >
            Continue without using this MCP server
          </button>
        </div>
      </div>
    </div>
  );
}
