import { CheckSquare, Square } from 'lucide-react';

export interface MessageSelectorProps {
  selected: boolean;
  messageId: string;
  onToggle: (messageId: string) => void;
}

export function MessageSelector({ selected, messageId, onToggle }: MessageSelectorProps) {
  return (
    <button
      type="button"
      data-testid={`message-selector-${messageId}`}
      className="inline-flex items-center p-0.5 text-slate-400 hover:text-slate-600"
      onClick={() => onToggle(messageId)}
      title={selected ? '取消选择' : '选择消息'}
    >
      {selected ? (
        <CheckSquare className="h-4 w-4 text-blue-500" />
      ) : (
        <Square className="h-4 w-4" />
      )}
    </button>
  );
}
