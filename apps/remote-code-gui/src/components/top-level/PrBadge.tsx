import { GitPullRequest } from 'lucide-react';

export interface PrBadgeProps {
  prNumber: number;
  prTitle?: string;
  url?: string;
}

export function PrBadge({ prNumber, prTitle, url }: PrBadgeProps) {
  const content = (
    <span data-testid="pr-badge" className="inline-flex items-center gap-1.5 rounded-full border border-purple-200 bg-purple-50 px-2.5 py-0.5 text-xs font-medium text-purple-700">
      <GitPullRequest className="h-3.5 w-3.5" />
      #{prNumber}
      {prTitle && <span className="max-w-[200px] truncate">{prTitle}</span>}
    </span>
  );

  if (url) {
    return (
      <a href={url} target="_blank" rel="noopener noreferrer" className="inline-block">
        {content}
      </a>
    );
  }
  return content;
}
