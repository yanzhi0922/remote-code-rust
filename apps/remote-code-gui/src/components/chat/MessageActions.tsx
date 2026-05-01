import { useState } from 'react';
import { Copy, Check, RotateCcw, Pencil } from 'lucide-react';

interface MessageActionsProps {
  content: string;
  onRetry?: () => void;
  onEdit?: () => void;
  className?: string;
}

export function MessageActions({ content, onRetry, onEdit, className = '' }: MessageActionsProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback for environments without clipboard API
    }
  };

  return (
    <div className={`flex items-center gap-1 ${className}`}>
      <button
        title="复制"
        onClick={() => void handleCopy()}
        className="flex h-7 w-7 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
      >
        {copied ? <Check size={14} className="text-rc-accent-success" /> : <Copy size={14} />}
      </button>
      {onRetry && (
        <button
          title="重试"
          onClick={onRetry}
          className="flex h-7 w-7 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          <RotateCcw size={14} />
        </button>
      )}
      {onEdit && (
        <button
          title="编辑"
          onClick={onEdit}
          className="flex h-7 w-7 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          <Pencil size={14} />
        </button>
      )}
    </div>
  );
}
