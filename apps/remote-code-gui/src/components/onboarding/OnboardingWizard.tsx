/**
 * OnboardingWizard — 新手引导向导。
 *
 * 4 步向导：欢迎 → API Key → 模型选择 → 完成。
 * 包含步骤指示器、跳过按钮和导航控制。
 */

import { useState } from 'react';
import { ChevronLeft, ChevronRight, SkipForward } from 'lucide-react';

export interface OnboardingWizardProps {
  visible: boolean;
  onComplete: () => void;
}

const STEPS = ['欢迎', 'API Key', '模型选择', '完成'] as const;

const MODEL_OPTIONS = [
  { value: 'gpt-4o', label: 'GPT-4o' },
  { value: 'gpt-4o-mini', label: 'GPT-4o Mini' },
  { value: 'claude-sonnet-4-20250514', label: 'Claude Sonnet 4' },
  { value: 'claude-haiku-35-20241022', label: 'Claude 3.5 Haiku' },
  { value: 'gemini-2.5-pro', label: 'Gemini 2.5 Pro' },
];

export function OnboardingWizard({ visible, onComplete }: OnboardingWizardProps) {
  const [step, setStep] = useState(0);
  const [apiKey, setApiKey] = useState('');
  const [model, setModel] = useState('gpt-4o');

  if (!visible) return null;

  const handleNext = () => {
    if (step < STEPS.length - 1) {
      setStep(step + 1);
    } else {
      onComplete();
    }
  };

  const handlePrev = () => {
    if (step > 0) setStep(step - 1);
  };

  const handleSkip = () => {
    onComplete();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      data-testid="onboarding-overlay"
    >
      <div className="w-full max-w-lg rounded-2xl bg-white p-6 shadow-xl" data-testid="onboarding-wizard">
        {/* Step indicator */}
        <div className="flex items-center justify-between" data-testid="step-indicator">
          {STEPS.map((label, i) => (
            <div key={label} className="flex items-center">
              <div
                className={`flex h-8 w-8 items-center justify-center rounded-full text-xs font-medium ${
                  i <= step
                    ? 'bg-blue-600 text-white'
                    : 'bg-slate-200 text-slate-500'
                }`}
                data-testid={`step-dot-${i}`}
              >
                {i + 1}
              </div>
              <span
                className={`ml-2 hidden text-sm sm:inline ${
                  i <= step ? 'font-medium text-slate-900' : 'text-slate-400'
                }`}
              >
                {label}
              </span>
              {i < STEPS.length - 1 && (
                <div
                  className={`mx-2 h-0.5 w-6 sm:w-10 ${
                    i < step ? 'bg-blue-600' : 'bg-slate-200'
                  }`}
                />
              )}
            </div>
          ))}
        </div>

        {/* Step content */}
        <div className="mt-8 min-h-[180px]" data-testid="step-content">
          {step === 0 && (
            <div data-testid="step-welcome">
              <h2 className="text-xl font-semibold text-slate-900">欢迎使用 Remote Code</h2>
              <p className="mt-3 text-sm leading-6 text-slate-600">
                让我们快速设置您的开发环境。只需几步即可开始使用 AI 辅助编程。
              </p>
            </div>
          )}

          {step === 1 && (
            <div data-testid="step-apikey">
              <h2 className="text-xl font-semibold text-slate-900">配置 API Key</h2>
              <p className="mt-2 text-sm text-slate-600">输入您的 AI 提供商 API Key。</p>
              <input
                type="password"
                className="mt-4 w-full rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-900 placeholder-slate-400 focus:border-blue-500 focus:outline-none"
                placeholder="sk-..."
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                data-testid="apikey-input"
              />
            </div>
          )}

          {step === 2 && (
            <div data-testid="step-model">
              <h2 className="text-xl font-semibold text-slate-900">选择模型</h2>
              <p className="mt-2 text-sm text-slate-600">选择您想使用的 AI 模型。</p>
              <select
                className="mt-4 w-full rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-900 focus:border-blue-500 focus:outline-none"
                value={model}
                onChange={(e) => setModel(e.target.value)}
                data-testid="model-select"
                aria-label="选择模型"
              >
                {MODEL_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </div>
          )}

          {step === 3 && (
            <div data-testid="step-done">
              <h2 className="text-xl font-semibold text-slate-900">设置完成！</h2>
              <p className="mt-3 text-sm leading-6 text-slate-600">
                一切准备就绪。点击"完成"开始使用 Remote Code。
              </p>
            </div>
          )}
        </div>

        {/* Navigation */}
        <div className="mt-6 flex items-center justify-between">
          <button
            onClick={handleSkip}
            className="flex items-center gap-1 text-sm text-slate-400 hover:text-slate-600"
            data-testid="skip-button"
          >
            <SkipForward className="h-4 w-4" />
            跳过
          </button>

          <div className="flex gap-2">
            {step > 0 && (
              <button
                onClick={handlePrev}
                className="flex items-center gap-1 rounded-xl border border-slate-200 px-4 py-2 text-sm text-slate-600 hover:bg-slate-50"
                data-testid="prev-button"
              >
                <ChevronLeft className="h-4 w-4" />
                上一步
              </button>
            )}
            <button
              onClick={handleNext}
              className="flex items-center gap-1 rounded-xl bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
              data-testid="next-button"
            >
              {step === STEPS.length - 1 ? '完成' : '下一步'}
              {step < STEPS.length - 1 && <ChevronRight className="h-4 w-4" />}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
