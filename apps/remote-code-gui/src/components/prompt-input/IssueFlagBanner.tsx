import { Flag, X } from 'lucide-react';

export interface IssueFlagBannerProps {
  issue: string;
  onDismiss?: () => void;
}

export function IssueFlagBanner({ issue, onDismiss }: IssueFlagBannerProps) {
  return (
    <div data-testid="issue-flag-banner" className="flex items-center gap-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2">
      <Flag className="h-4 w-4 shrink-0 text-amber-600" />
      <span className="flex-1 text-sm text-amber-700">{issue}</span>
      {onDismiss && (
        <button
          type="button"
          data-testid="issue-flag-dismiss"
          className="rounded p-0.5 hover:bg-amber-100"
          onClick={onDismiss}
          title="关闭"
        >
          <X className="h-3.5 w-3.5 text-amber-400" />
        </button>
      )}
    </div>
  );
}
