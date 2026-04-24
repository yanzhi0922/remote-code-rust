import { useState, useEffect } from 'react';
import { ChevronDown, ChevronUp, ShieldCheck, ShieldAlert, ShieldX } from 'lucide-react';

export type RiskLevel = 'LOW' | 'MEDIUM' | 'HIGH';

export interface PermissionExplanationProps {
  toolName: string;
  toolInput: unknown;
  visible: boolean;
  onToggle: () => void;
}

interface ExplanationResult {
  riskLevel: RiskLevel;
  explanation: string;
}

function getRiskColor(risk: RiskLevel): string {
  switch (risk) {
    case 'LOW':
      return 'text-emerald-600 bg-emerald-50';
    case 'MEDIUM':
      return 'text-amber-600 bg-amber-50';
    case 'HIGH':
      return 'text-red-600 bg-red-50';
  }
}

function getRiskIcon(risk: RiskLevel) {
  switch (risk) {
    case 'LOW':
      return ShieldCheck;
    case 'MEDIUM':
      return ShieldAlert;
    case 'HIGH':
      return ShieldX;
  }
}

function getRiskLabel(risk: RiskLevel): string {
  switch (risk) {
    case 'LOW':
      return 'Low risk';
    case 'MEDIUM':
      return 'Medium risk';
    case 'HIGH':
      return 'High risk';
  }
}

function simulateExplanation(toolName: string, _toolInput: unknown): ExplanationResult {
  const inputStr = typeof _toolInput === 'string' ? _toolInput : JSON.stringify(_toolInput);
  const hasDangerousKeywords = /rm\s|del\s|format\s|drop\s|truncate/i.test(inputStr ?? '');
  const hasNetworkKeywords = /curl|wget|fetch|http/i.test(inputStr ?? '');

  if (hasDangerousKeywords) {
    return {
      riskLevel: 'HIGH',
      explanation: `The "${toolName}" tool will execute a potentially destructive operation. This may modify or delete data irreversibly. Please review carefully before approving.`,
    };
  }
  if (hasNetworkKeywords) {
    return {
      riskLevel: 'MEDIUM',
      explanation: `The "${toolName}" tool involves network access. This could expose data to external services or download untrusted content.`,
    };
  }
  return {
    riskLevel: 'LOW',
    explanation: `The "${toolName}" tool appears to perform a standard read-only or safe operation.`,
  };
}

export function PermissionExplanation({
  toolName,
  toolInput,
  visible,
  onToggle,
}: PermissionExplanationProps) {
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<ExplanationResult | null>(null);

  useEffect(() => {
    if (visible && !result) {
      setLoading(true);
      const timer = setTimeout(() => {
        setResult(simulateExplanation(toolName, toolInput));
        setLoading(false);
      }, 500);
      return () => clearTimeout(timer);
    }
  }, [visible, toolName, toolInput, result]);

  if (!visible) {
    return (
      <button
        type="button"
        onClick={onToggle}
        className="flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-sm text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-700"
      >
        <ChevronDown size={14} />
        <span>查看风险说明 (Ctrl+E)</span>
      </button>
    );
  }

  return (
    <div className="rounded-xl border border-slate-200 bg-slate-50 p-3">
      <button
        type="button"
        onClick={onToggle}
        className="mb-2 flex items-center gap-1.5 text-sm text-slate-500 hover:text-slate-700"
      >
        <ChevronUp size={14} />
        <span>收起风险说明</span>
      </button>

      {loading && (
        <div className="space-y-2" data-testid="explanation-loading">
          <div className="h-4 w-24 animate-pulse rounded bg-slate-200" />
          <div className="h-3 w-full animate-pulse rounded bg-slate-200" />
          <div className="h-3 w-3/4 animate-pulse rounded bg-slate-200" />
        </div>
      )}

      {result && (
        <div data-testid="explanation-result">
          <div className="mb-2 flex items-center gap-2">
            {(() => {
              const Icon = getRiskIcon(result.riskLevel);
              return (
                <span
                  className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-semibold ${getRiskColor(result.riskLevel)}`}
                >
                  <Icon size={12} />
                  {getRiskLabel(result.riskLevel)}
                </span>
              );
            })()}
          </div>
          <p className="text-sm text-slate-600">{result.explanation}</p>
        </div>
      )}
    </div>
  );
}
