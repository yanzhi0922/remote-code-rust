import { useState } from 'react';
import { Sparkles } from 'lucide-react';
import { cn } from '../../../lib/utils';

export interface PromptStepProps {
  value: string;
  onChange: (prompt: string) => void;
  className?: string;
}

const TEMPLATE_SUGGESTIONS = [
  {
    label: '代码审查助手',
    prompt: '你是一个专业的代码审查助手。请仔细分析代码变更，找出潜在的 bug、安全漏洞和性能问题，并提供具体的改进建议。',
  },
  {
    label: '文档生成器',
    prompt: '你是一个技术文档专家。请根据代码和上下文，生成清晰、准确的技术文档，包括 API 说明、使用示例和注意事项。',
  },
  {
    label: '测试工程师',
    prompt: '你是一个测试工程师。请为给定的代码生成全面的测试用例，覆盖正常路径、边界条件和异常情况。',
  },
];

export function PromptStep({ value, onChange, className }: PromptStepProps) {
  const [showTemplates, setShowTemplates] = useState(false);

  return (
    <div data-testid="wizard-prompt-step" className={cn('space-y-3', className)}>
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-slate-700">系统提示词</h3>
        <button
          type="button"
          data-testid="toggle-templates"
          onClick={() => setShowTemplates(!showTemplates)}
          className="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-blue-600 hover:bg-blue-50"
        >
          <Sparkles className="h-3 w-3" />
          模板建议
        </button>
      </div>

      {showTemplates && (
        <div data-testid="template-suggestions" className="flex flex-wrap gap-2">
          {TEMPLATE_SUGGESTIONS.map((tpl) => (
            <button
              key={tpl.label}
              type="button"
              data-testid={`template-${tpl.label}`}
              onClick={() => onChange(tpl.prompt)}
              className="rounded-full border border-blue-200 bg-blue-50 px-3 py-1 text-xs text-blue-700 transition-colors hover:bg-blue-100"
            >
              {tpl.label}
            </button>
          ))}
        </div>
      )}

      <textarea
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="输入系统提示词，定义 Agent 的行为和角色..."
        rows={8}
        data-testid="prompt-input"
        className="w-full resize-y rounded-lg border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-800 placeholder:text-slate-400 focus:border-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-200"
      />
      <p className="text-right text-xs text-slate-400">
        {value.length} 字符
      </p>
    </div>
  );
}
