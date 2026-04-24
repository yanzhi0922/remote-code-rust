import { Cloud, ExternalLink } from 'lucide-react';

export interface RemoteCalloutProps {
  host: string;
  environment?: string;
}

export function RemoteCallout({ host, environment }: RemoteCalloutProps) {
  return (
    <div data-testid="remote-callout" className="flex items-center gap-2 rounded-lg border border-blue-200 bg-blue-50 px-3 py-2">
      <Cloud className="h-4 w-4 shrink-0 text-blue-600" />
      <div className="flex-1">
        <span className="text-sm font-medium text-blue-700">远程环境</span>
        <span className="ml-2 text-xs text-blue-600">{host}</span>
        {environment && (
          <span className="ml-2 rounded bg-blue-100 px-1.5 py-0.5 text-xs text-blue-700">{environment}</span>
        )}
      </div>
      <ExternalLink className="h-3.5 w-3.5 text-blue-400" />
    </div>
  );
}
