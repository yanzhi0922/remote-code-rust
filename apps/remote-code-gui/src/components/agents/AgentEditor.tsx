import { useState } from 'react';
import { Save } from 'lucide-react';
import { ColorPicker } from './ColorPicker';
import { ModelSelector } from './ModelSelector';
import { ToolSelector } from './ToolSelector';
import type { AgentFormData } from './AgentFormData';
import type { ModelOption } from './AgentFormData';

export interface AgentEditorProps {
  agent?: {
    name: string;
    description: string;
    model?: string;
    color?: string;
    system_prompt: string;
    tools: string[];
    disabled: boolean;
  };
  onSave: (agent: AgentFormData) => void;
  onCancel: () => void;
  availableTools: string[];
  models?: ModelOption[];
}

export function AgentEditor({ agent, onSave, onCancel, availableTools, models = [] }: AgentEditorProps) {
  const isEditing = !!agent;

  const [name, setName] = useState(agent?.name ?? '');
  const [description, setDescription] = useState(agent?.description ?? '');
  const [model, setModel] = useState<string | null>(agent?.model ?? null);
  const [color, setColor] = useState(agent?.color ?? '#6b7280');
  const [systemPrompt, setSystemPrompt] = useState(agent?.system_prompt ?? '');
  const [selectedTools, setSelectedTools] = useState<string[]>(agent?.tools ?? []);
  const [disabled, setDisabled] = useState(agent?.disabled ?? false);
  const [errors, setErrors] = useState<{ name?: string; system_prompt?: string }>({});

  function handleToolToggle(tool: string) {
    setSelectedTools((prev) =>
      prev.includes(tool) ? prev.filter((t) => t !== tool) : [...prev, tool],
    );
  }

  function handleSubmit() {
    const nextErrors: typeof errors = {};
    if (!name.trim()) {
      nextErrors.name = '名称不能为空';
    }
    if (!systemPrompt.trim()) {
      nextErrors.system_prompt = '系统提示词不能为空';
    }
    if (Object.keys(nextErrors).length > 0) {
      setErrors(nextErrors);
      return;
    }
    setErrors({});
    onSave({
      name: name.trim(),
      description: description.trim(),
      model,
      color,
      system_prompt: systemPrompt.trim(),
      tools: selectedTools,
      disabled,
    });
  }

  return (
    <div className="space-y-5 rounded-2xl border border-slate-200 bg-white p-6" data-testid="agent-editor">
      <h2 className="text-lg font-semibold text-slate-800">
        {isEditing ? `编辑 Agent: ${agent.name}` : '创建 Agent'}
      </h2>

      {/* 名称 */}
      <div>
        <label htmlFor="agent-name" className="mb-1 block text-sm font-medium text-slate-700">
          名称 <span className="text-red-500">*</span>
        </label>
        <input
          id="agent-name"
          type="text"
          value={name}
          onChange={(e) => {
            setName(e.target.value);
            if (errors.name) setErrors((prev) => ({ ...prev, name: undefined }));
          }}
          readOnly={isEditing}
          placeholder="my-agent"
          className={`w-full rounded-xl border px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-blue-500 ${
            isEditing
              ? 'border-slate-200 bg-slate-50 text-slate-500'
              : 'border-slate-300 focus:border-blue-500'
          }`}
        />
        {errors.name && <span className="mt-1 text-xs text-red-500">{errors.name}</span>}
      </div>

      {/* 描述 */}
      <div>
        <label htmlFor="agent-description" className="mb-1 block text-sm font-medium text-slate-700">
          描述
        </label>
        <input
          id="agent-description"
          type="text"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Agent 的简短描述"
          className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
        />
      </div>

      {/* 颜色 */}
      <ColorPicker value={color} onChange={setColor} />

      {/* 模型 */}
      {models.length > 0 && (
        <ModelSelector value={model} onChange={setModel} models={models} />
      )}

      {/* 系统提示词 */}
      <div>
        <label htmlFor="agent-system-prompt" className="mb-1 block text-sm font-medium text-slate-700">
          系统提示词 <span className="text-red-500">*</span>
        </label>
        <textarea
          id="agent-system-prompt"
          value={systemPrompt}
          onChange={(e) => {
            setSystemPrompt(e.target.value);
            if (errors.system_prompt) setErrors((prev) => ({ ...prev, system_prompt: undefined }));
          }}
          placeholder="你是一个..."
          rows={5}
          className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
        />
        {errors.system_prompt && (
          <span className="mt-1 text-xs text-red-500">{errors.system_prompt}</span>
        )}
      </div>

      {/* 工具选择 */}
      <ToolSelector
        selectedTools={selectedTools}
        onToggle={handleToolToggle}
        availableTools={availableTools}
      />

      {/* 禁用开关 */}
      <label className="flex items-center gap-2 text-sm text-slate-700">
        <input
          type="checkbox"
          checked={disabled}
          onChange={(e) => setDisabled(e.target.checked)}
          className="rounded border-slate-300 text-blue-600 focus:ring-blue-500"
        />
        禁用此 Agent
      </label>

      {/* 操作按钮 */}
      <div className="flex items-center justify-end gap-3 border-t border-slate-100 pt-4">
        <button
          type="button"
          onClick={onCancel}
          className="rounded-xl border border-slate-300 px-4 py-2 text-sm font-medium text-slate-600 hover:bg-slate-50"
        >
          取消
        </button>
        <button
          type="button"
          onClick={handleSubmit}
          className="flex items-center gap-1.5 rounded-xl bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
        >
          <Save className="h-4 w-4" />
          {isEditing ? '保存' : '创建'}
        </button>
      </div>
    </div>
  );
}
