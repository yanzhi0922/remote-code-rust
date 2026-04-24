import { ShieldAlert } from 'lucide-react';

export interface SandboxViolation {
  path: string;
  operation: string;
  denied: boolean;
}

export interface SandboxViolationExpandedViewProps {
  violations: SandboxViolation[];
}

export function SandboxViolationExpandedView({ violations }: SandboxViolationExpandedViewProps) {
  if (violations.length === 0) return null;

  return (
    <div data-testid="sandbox-violation-expanded" className="rounded-lg border border-red-200 bg-red-50 p-4">
      <div className="mb-3 flex items-center gap-2 text-red-700">
        <ShieldAlert className="h-5 w-5" />
        <h3 className="text-sm font-semibold">沙箱违规 ({violations.length})</h3>
      </div>
      <div className="space-y-2">
        {violations.map((v, i) => (
          <div key={i} data-testid={`sandbox-violation-${i}`} className="rounded bg-white p-2 text-sm">
            <div className="flex items-center gap-2">
              <span className={`text-xs font-medium ${v.denied ? 'text-red-600' : 'text-amber-600'}`}>
                {v.denied ? '已拒绝' : '警告'}
              </span>
              <span className="text-slate-700">{v.operation}</span>
            </div>
            <p className="mt-0.5 text-xs text-slate-500">{v.path}</p>
          </div>
        ))}
      </div>
    </div>
  );
}
