import { Clock } from 'lucide-react';

export interface MessageTimestampProps {
  timestamp: string;
}

export function MessageTimestamp({ timestamp }: MessageTimestampProps) {
  return (
    <span data-testid="message-timestamp" className="inline-flex items-center gap-1 text-xs text-slate-400">
      <Clock className="h-3 w-3" />
      {timestamp}
    </span>
  );
}
