import { useState } from 'react';
import { ChevronLeft, ChevronRight, Check } from 'lucide-react';
import { ColorPicker } from './ColorPicker';
import { ModelSelector } from './ModelSelector';
import { ToolSelector } from './ToolSelector';
import type { AgentFormData } from './AgentFormData';
import type { ModelOption } from './AgentFormData';

export interface AgentCreateWizardProps {
  onComplete: (agent: AgentFormData) => void;
  onCancel: () => void;
  availableTools: string[];
  models?: ModelOption[];
}

const STEPS = [
  { key: 'basic', label: '基本信息' },
  { key: 'model', label: '模型和行为' },
  { key: 'tools', label: '工具选择' },
] as const;

export function AgentCreateWizard({ onComplete, onCancel, availableTools, models = [] }: AgentCreateWizardProps) {
  const [step, setStep] = useState(0);

  // Step 1: Basic info
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [color, setColor] = useState('#6b7280');

  // Step 2: Model & behavior
  const [model, setModel] = useState<string | null>(null);
  const [systemPrompt, setSystemPrompt] = useState('');

  // Step 3: Tools
  const [selectedTools, setSelectedTools] = useState<string[]>([]);

  const [errors, setErrors] = useState<Record<string, string>>({});

  function validateStep(s: number): boolean {
    const nextErrors: Record<string, string> = {};
    if (s === 0) {
      if (!name.trim()) nextErrors.name = '名称不能为空';
    }
    if (s === 1) {
      if (!systemPrompt.trim()) nextErrors.system_prompt = '系统提示词不能为空';
    }
    setErrors(nextErrors);
    return Object.keys(nextErrors).length === 0;
  }

  function handleNext() {
    if (!validateStep(step)) return;
    setStep((prev) => Math.min(prev + 1, STEPS.length - 1));
  }

  function handlePrev() {
    setErrors({});
    setStep((prev) => Math.max(prev - 1, 0));
  }

  function handleComplete() {
    if (!validateStep(step)) return;
    onComplete({
      name: name.trim(),
      description: description.trim(),
      model,
      color,
      system_prompt: systemPrompt.trim(),
      tools: selectedTools,
      disabled: false,
    });
  }

  function handleToolToggle(tool: string) {
    setSelectedTools((prev) =>
      prev.includes(tool) ? prev.filter((t) => t !== tool) : [...prev, tool],
    );
  }

  return (
    <div className="space-y-6 rounded-2xl border border-slate-200 bg-white p-6" data-testid="agent-create-wizard">
      <h2 className="text-lg font-semibold text-slate-800">创建 Agent</h2>

      {/* 步骤指示器 */}
      <div className="flex items-center gap-2">
        {STEPS.map((s, i) => (
          <div key={s.key} className="flex items-center gap-2">
            <div
              className={`flex h-8 w-8 items-center justify-center rounded-full text-xs font-bold ${
                i < step
                  ? 'bg-blue-600 text-white'
                  : i === step
                    ? 'bg-blue-600 text-white ring-2 ring-blue-200'
                    : 'bg-slate-100 text-slate-400'
              }`}
            >
              {i < step ? <Check className="h-4 w-4" /> : i + 1}
            </div>
            <span
              className={`text-sm ${
                i === step ? 'font-medium text-slate-800' : 'text-slate-400'
              }`}
            >
              {s.label}
            </span>
            {i < STEPS.length - 1 && (
              <div className="mx-1 h-px w-6 bg-slate-200" />
            )}
          </div>
        ))}
      </div>

      {/* 步骤内容 */}
      <div className="min-h-[300px]">
        {step === 0 && (
          <div className="space-y-4">
            <div>
              <label htmlFor="wizard-name" className="mb-1 block text-sm font-medium text-slate-700">
                名称 <span className="text-red-500">*</span>
              </label>
              <input
                id="wizard-name"
                type="text"
                value={name}
                onChange={(e) => {
                  setName(e.target.value);
                  if (errors.name) setErrors((prev) => ({ ...prev, name: '' }));
                }}
                placeholder="my-agent"
                className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
              {errors.name && <span className="mt-1 text-xs text-red-500">{errors.name}</span>}
            </div>

            <div>
              <label htmlFor="wizard-description" className="mb-1 block text-sm font-medium text-slate-700">
                描述
              </label>
              <input
                id="wizard-description"
                type="text"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Agent 的简短描述"
                className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
            </div>

            <ColorPicker value={color} onChange={setColor} />
          </div>
        )}

        {step === 1 && (
          <div className="space-y-4">
            {models.length > 0 && (
              <ModelSelector value={model} onChange={setModel} models={models} />
            )}

            <div>
              <label htmlFor="wizard-prompt" className="mb-1 block text-sm font-medium text-slate-700">
                系统提示词 <span className="text-red-500">*</span>
              </label>
              <textarea
                id="wizard-prompt"
                value={systemPrompt}
                onChange={(e) => {
                  setSystemPrompt(e.target.value);
                  if (errors.system_prompt) setErrors((prev) => ({ ...prev, system_prompt: '' }));
                }}
                placeholder="你是一个..."
                rows={8}
                className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
              {errors.system_prompt && (
                <span className="mt-1 text-xs text-red-500">{errors.system_prompt}</span>
              )}
            </div>
          </div>
        )}

        {step === 2 && (
          <ToolSelector
            selectedTools={selectedTools}
            onToggle={handleToolToggle}
            availableTools={availableTools}
          />
        )}
      </div>

      {/* 导航按钮 */}
      <div className="flex items-center justify-between border-t border-slate-100 pt-4">
        <button
          type="button"
          onClick={step === 0 ? onCancel : handlePrev}
          className="flex items-center gap-1 rounded-xl border border-slate-300 px-4 py-2 text-sm font-medium text-slate-600 hover:bg-slate-50"
        >
          {step === 0 ? (
            '取消'
          ) : (
            <>
              <ChevronLeft className="h-4 w-4" />
              上一步
            </>
          )}
        </button>

        {step < STEPS.length - 1 ? (
          <button
            type="button"
            onClick={handleNext}
            className="flex items-center gap-1 rounded-xl bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
          >
            下一步
            <ChevronRight className="h-4 w-4" />
          </button>
        ) : (
          <button
            type="button"
            onClick={handleComplete}
            className="flex items-center gap-1 rounded-xl bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
          >
            <Check className="h-4 w-4" />
            完成
          </button>
        )}
      </div>
    </div>
  );
}
