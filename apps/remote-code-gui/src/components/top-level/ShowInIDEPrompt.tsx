import { ExternalLink } from 'lucide-react';

export interface ShowInIDEPromptProps {
  filePath: string;
  line?: number;
  onOpen?: () => void;
}

export function ShowInIDEPrompt({ filePath, line, onOpen }: ShowInIDEPromptProps) {
  return (
    <button
      type="button"
      data-testid="show-in-ide-prompt"
      className="inline-flex items-center gap-1.5 rounded px-2 py-1 text-xs text-blue-600 hover:bg-blue-50"
      onClick={onOpen}
    >
      <ExternalLink className="h-3.5 w-3.5" />
      <span>在IDE中打开 {filePath}{line ? `:${line}` : ''}</span>
    </button>
  );
}
