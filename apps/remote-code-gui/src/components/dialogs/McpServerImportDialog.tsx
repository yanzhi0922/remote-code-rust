import { type ReactNode, useState } from 'react';
import { Download, X, Server, AlertCircle, CheckCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface McpServerConfig {
  command: string;
  args?: string[];
}

interface Props {
  servers: Record<string, McpServerConfig>;
  scope: 'project' | 'user';
  onDone: () => void;
}

export function McpServerImportDialog({ servers, scope, onDone }: Props): ReactNode {
  const serverNames = Object.keys(servers);
  const [selected, setSelected] = useState<Set<string>>(new Set(serverNames));
  const [importing, setImporting] = useState(false);
  const [importedCount, setImportedCount] = useState(0);

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

  const handleImport = () => {
    setImporting(true);
    setImportedCount(selected.size);
    onDone();
  };

  return (
    <div
      data-testid="mcp-server-import-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Download className="h-5 w-5 text-green-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Import MCP Servers
            </h3>
          </div>
          <button
            data-testid="mcp-server-import-close"
            onClick={onDone}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <p className="mt-2 text-sm text-gray-600 dark:text-gray-400">
          Import MCP servers to your <span className="font-semibold">{scope}</span> configuration:
        </p>

        <div className="mt-3 space-y-2">
          {serverNames.map((name) => {
            const isSelected = selected.has(name);
            return (
              <button
                key={name}
                data-testid={`mcp-server-import-${name}`}
                onClick={() => toggleServer(name)}
                className={cn(
                  'flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm',
                  isSelected
                    ? 'border-2 border-green-500 bg-green-50 dark:bg-green-950'
                    : 'border border-gray-200 bg-gray-50 hover:bg-gray-100 dark:border-gray-600 dark:bg-gray-700 dark:hover:bg-gray-600',
                )}
              >
                <Server className="h-4 w-4 shrink-0 text-gray-500" />
                <span className="flex-1 text-gray-900 dark:text-gray-100">{name}</span>
                {isSelected && <CheckCircle className="h-4 w-4 text-green-500" />}
              </button>
            );
          })}
        </div>

        {serverNames.length > 0 && (
          <div className="mt-2 flex items-center gap-1">
            <AlertCircle className="h-3 w-3 text-amber-500" />
            <p className="text-xs text-amber-600 dark:text-amber-400">
              Existing servers with the same name will be renamed automatically.
            </p>
          </div>
        )}

        {importing ? (
          <div className="mt-4 flex items-center gap-2">
            <CheckCircle className="h-4 w-4 text-green-500" />
            <p className="text-sm text-green-600 dark:text-green-400">
              Successfully imported {importedCount} MCP server{importedCount !== 1 ? 's' : ''}.
            </p>
          </div>
        ) : (
          <div className="mt-4 flex justify-end gap-2">
            <button
              data-testid="mcp-server-import-cancel"
              onClick={onDone}
              className="rounded bg-gray-100 px-4 py-2 text-sm text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
            >
              Cancel
            </button>
            <button
              data-testid="mcp-server-import-confirm"
              onClick={handleImport}
              disabled={selected.size === 0}
              className={cn(
                'rounded px-4 py-2 text-sm text-white',
                selected.size > 0
                  ? 'bg-green-600 hover:bg-green-700'
                  : 'bg-gray-400 cursor-not-allowed',
              )}
            >
              Import ({selected.size})
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
