import { useState } from 'react';
import { Copy, Check } from 'lucide-react';

export interface McpServerDialogCopyProps {
  text: string;
}

export function McpServerDialogCopy({ text }: McpServerDialogCopyProps) {
  const [copied, setCopied] = useState(false);

  function handleCopy() {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }

  return (
    <button
      type="button"
      data-testid="mcp-server-dialog-copy"
      className="inline-flex items-center gap-1 rounded px-2 py-1 text-xs text-slate-500 hover:bg-slate-100"
      onClick={handleCopy}
      title="复制"
    >
      {copied ? (
        <>
          <Check className="h-3.5 w-3.5 text-green-500" />
          <span className="text-green-500">已复制</span>
        </>
      ) : (
        <>
          <Copy className="h-3.5 w-3.5" />
          <span>复制</span>
        </>
      )}
    </button>
  );
}
