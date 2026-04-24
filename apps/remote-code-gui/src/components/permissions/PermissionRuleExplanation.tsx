import { Shield, CheckCircle2, XCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

/* ------------------------------------------------------------------ */
/* Types                                                               */
/* ------------------------------------------------------------------ */

export interface PermissionRule {
  tool: string;
  behavior: string;
  pattern?: string;
}

export interface PermissionRuleExplanationProps {
  rule: PermissionRule;
  className?: string;
}

/* ------------------------------------------------------------------ */
/* Helpers                                                             */
/* ------------------------------------------------------------------ */

function isAllowBehavior(behavior: string): boolean {
  return behavior.toLowerCase() === 'allow';
}

function formatRuleExplanation(rule: PermissionRule): string {
  const verb = isAllowBehavior(rule.behavior) ? 'Allow' : 'Deny';
  const toolLabel = rule.tool;
  if (rule.pattern) {
    return `${verb} ${toolLabel} matching '${rule.pattern}'`;
  }
  return `${verb} ${toolLabel}`;
}

/* ------------------------------------------------------------------ */
/* Component                                                           */
/* ------------------------------------------------------------------ */

export function PermissionRuleExplanation({
  rule,
  className,
}: PermissionRuleExplanationProps) {
  const allowed = isAllowBehavior(rule.behavior);
  const explanation = formatRuleExplanation(rule);

  return (
    <div
      className={cn(
        'flex items-center gap-2 rounded-lg px-3 py-2 text-sm',
        allowed ? 'bg-emerald-50 text-emerald-700' : 'bg-red-50 text-red-700',
        className,
      )}
      data-testid="permission-rule-explanation"
    >
      {allowed ? (
        <CheckCircle2 size={14} className="shrink-0" />
      ) : (
        <XCircle size={14} className="shrink-0" />
      )}
      <Shield size={14} className="shrink-0 opacity-60" />
      <span className="font-medium">{explanation}</span>
    </div>
  );
}
