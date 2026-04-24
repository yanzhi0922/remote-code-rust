export interface PermissionDecisionDebugInfoProps {
  decision: {
    classifier: string;
    rule?: string;
    autoApproved: boolean;
    checkInProgress: boolean;
  };
  verbose?: boolean;
}

export function PermissionDecisionDebugInfo({
  decision,
  verbose = false,
}: PermissionDecisionDebugInfoProps) {
  if (!verbose) return null;

  return (
    <div
      className="mt-2 rounded-lg border border-slate-200 bg-slate-50 p-2 font-mono text-xs text-slate-500"
      data-testid="debug-info"
    >
      <div className="flex flex-col gap-1">
        <div>
          <span className="font-semibold text-slate-600">Classifier:</span>{' '}
          {decision.classifier}
        </div>
        {decision.rule && (
          <div>
            <span className="font-semibold text-slate-600">Rule:</span>{' '}
            {decision.rule}
          </div>
        )}
        <div>
          <span className="font-semibold text-slate-600">Auto-approved:</span>{' '}
          {decision.autoApproved ? 'Yes' : 'No'}
        </div>
        <div>
          <span className="font-semibold text-slate-600">Check in progress:</span>{' '}
          {decision.checkInProgress ? 'Yes' : 'No'}
        </div>
      </div>
    </div>
  );
}
