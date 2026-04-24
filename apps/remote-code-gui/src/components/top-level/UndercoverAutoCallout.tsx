import { EyeOff } from 'lucide-react';

export interface UndercoverAutoCalloutProps {
  active: boolean;
  onDismiss?: () => void;
}

export function UndercoverAutoCallout({ active, onDismiss }: UndercoverAutoCalloutProps) {
  if (!active) return null;

  return (
    <div data-testid="undercover-auto-callout" className="flex items-center gap-2 rounded-lg border border-slate-200 bg-slate-50 px-3 py-2">
      <EyeOff className="h-4 w-4 text-slate-500" />
      <span className="text-sm text-slate-600">隐蔽模式已激活</span>
      {onDismiss && (
        <button type="button" className="ml-auto text-xs text-slate-400 hover:text-slate-600" onClick={onDismiss}>
          关闭
        </button>
      )}
    </div>
  );
}
