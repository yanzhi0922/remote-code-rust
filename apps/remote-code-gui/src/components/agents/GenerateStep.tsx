import { Sparkles, Loader2 } from 'lucide-react';

export interface GenerateStepProps {
  generating: boolean;
  prompt: string;
  result?: string;
  onGenerate?: () => void;
}

export function GenerateStep({ generating, prompt, result, onGenerate }: GenerateStepProps) {
  return (
    <div data-testid="generate-step" className="space-y-3">
      <div className="flex items-center gap-2">
        <Sparkles className="h-4 w-4 text-purple-500" />
        <h3 className="text-sm font-semibold text-slate-800">生成步骤</h3>
      </div>
      <div className="rounded bg-slate-50 p-3">
        <p className="text-sm text-slate-600">{prompt}</p>
      </div>
      {generating ? (
        <div data-testid="generate-step-loading" className="flex items-center gap-2 text-sm text-slate-500">
          <Loader2 className="h-4 w-4 animate-spin" />
          正在生成...
        </div>
      ) : result ? (
        <div data-testid="generate-step-result" className="rounded border border-slate-200 bg-white p-3">
          <pre className="whitespace-pre-wrap text-sm text-slate-700">{result}</pre>
        </div>
      ) : (
        <button
          type="button"
          data-testid="generate-step-button"
          className="inline-flex items-center gap-1.5 rounded bg-purple-600 px-3 py-1.5 text-sm text-white hover:bg-purple-700"
          onClick={onGenerate}
        >
          <Sparkles className="h-3.5 w-3.5" />
          生成
        </button>
      )}
    </div>
  );
}
