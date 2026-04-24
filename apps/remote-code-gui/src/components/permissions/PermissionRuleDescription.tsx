export type PermissionBehavior = 'allow' | 'deny' | 'ask';

export interface PermissionRuleDescriptionProps {
  ruleValue: {
    tool_name: string;
    rule_content: string;
    behavior: PermissionBehavior;
  };
}

function getBehaviorBadgeClasses(behavior: PermissionBehavior): string {
  switch (behavior) {
    case 'allow':
      return 'bg-emerald-50 text-emerald-700';
    case 'deny':
      return 'bg-red-50 text-red-700';
    case 'ask':
      return 'bg-amber-50 text-amber-700';
  }
}

function getBehaviorLabel(behavior: PermissionBehavior): string {
  switch (behavior) {
    case 'allow':
      return 'Allow';
    case 'deny':
      return 'Deny';
    case 'ask':
      return 'Ask';
  }
}

function parseRuleDescription(toolName: string, ruleContent: string): string {
  if (!ruleContent) {
    return `Any use of the ${toolName} tool`;
  }

  // "prompt:" prefix for semantic rules
  if (ruleContent.startsWith('prompt:')) {
    return `Semantic rule: "${ruleContent.slice(7).trim()}"`;
  }

  // Glob pattern ending with :*
  if (ruleContent.endsWith(':*')) {
    const prefix = ruleContent.slice(0, -2);
    return `Any ${toolName} command starting with "${prefix}"`;
  }

  // File path glob patterns
  if (ruleContent.includes('*') || ruleContent.includes('?')) {
    return `${toolName} matching pattern "${ruleContent}"`;
  }

  // Exact match
  return `The ${toolName} command "${ruleContent}"`;
}

export function PermissionRuleDescription({ ruleValue }: PermissionRuleDescriptionProps) {
  const { tool_name, rule_content, behavior } = ruleValue;
  const description = parseRuleDescription(tool_name, rule_content);

  return (
    <div className="flex items-start gap-2">
      <span
        className={`inline-flex shrink-0 items-center rounded-full px-2 py-0.5 text-xs font-semibold ${getBehaviorBadgeClasses(behavior)}`}
      >
        {getBehaviorLabel(behavior)}
      </span>
      <span className="text-sm text-slate-600">{description}</span>
    </div>
  );
}
