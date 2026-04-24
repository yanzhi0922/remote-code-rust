import { type ReactNode, useState } from 'react';
import { Server, X, ShieldAlert, CheckCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface Props {
  serverNames: string[];
  onDone: (selected: string[]) => void;
}

export function McpServerMultiselectDialog({ serverNames, onDone }: Props): ReactNode {
  const [selected, setSelected] = useState<Set<string>>(new Set(serverNames));

  const toggleServer = (name: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) {
        next.delete(name);
      } else {
        next.add(name);
      }
      return next;
    });
  };

  const handleSubmit = () => {
    onDone(Array.from(selected));
  };

  const handleRejectAll = () => {
    onDone([]);
  };

  return (
    <div
      data-testid="mcp-server-multiselect-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Server className="h-5 w-5 text-blue-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              {serverNames.length} New MCP Servers Found
            </h3>
          </div>
          <button
            data-testid="mcp-server-multiselect-close"
            onClick={handleRejectAll}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <p className="mt-2 text-sm text-gray-600 dark:text-gray-400">
          {serverNames.length} new MCP servers found in .mcp.json
        </p>

        <div className="mt-2 flex items-start gap-1">
          <ShieldAlert className="mt-0.5 h-4 w-4 shrink-0 text-amber-500" />
          <p className="text-xs text-amber-600 dark:text-amber-400">
            Only use MCP servers from sources you trust. MCP servers can execute code and access your files.
          </p>
        </div>

        <div className="mt-3 space-y-2">
          {serverNames.map((name) => {
            const isSelected = selected.has(name);
            return (
              <button
                key={name}
                data-testid={`mcp-server-multiselect-${name}`}
                onClick={() => toggleServer(name)}
                className={cn(
                  'flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm',
                  isSelected
                    ? 'border-2 border-blue-500 bg-blue-50 dark:bg-blue-950'
                    : 'border border-gray-200 bg-gray-50 hover:bg-gray-100 dark:border-gray-600 dark:bg-gray-700 dark:hover:bg-gray-600',
                )}
              >
                <Server className="h-4 w-4 shrink-0 text-gray-500" />
                <span className="flex-1 text-gray-900 dark:text-gray-100">{name}</span>
                {isSelected && <CheckCircle className="h-4 w-4 text-blue-500" />}
              </button>
            );
          })}
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <button
            data-testid="mcp-server-multiselect-reject"
            onClick={handleRejectAll}
            className="rounded bg-gray-100 px-4 py-2 text-sm text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
          >
            Reject all
          </button>
          <button
            data-testid="mcp-server-multiselect-confirm"
            onClick={handleSubmit}
            className="rounded bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700"
          >
            Approve selected ({selected.size})
          </button>
        </div>
      </div>
    </div>
  );
}
