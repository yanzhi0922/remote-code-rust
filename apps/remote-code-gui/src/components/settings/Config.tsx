import { useState } from 'react';
import { Save, RotateCcw } from 'lucide-react';

export interface ConfigPanelProps {
  config: Record<string, unknown>;
  schema?: Array<{ key: string; label: string; type: 'string' | 'number' | 'boolean' }>;
  onSave?: (config: Record<string, unknown>) => void;
  onReset?: () => void;
}

export function Config({ config, schema = [], onSave, onReset }: ConfigPanelProps) {
  const [values, setValues] = useState<Record<string, unknown>>({ ...config });
  const [dirty, setDirty] = useState(false);

  function handleChange(key: string, value: unknown) {
    setValues((prev) => ({ ...prev, [key]: value }));
    setDirty(true);
  }

  function handleSave() {
    onSave?.(values);
    setDirty(false);
  }

  function handleReset() {
    setValues({ ...config });
    setDirty(false);
    onReset?.();
  }

  return (
    <div data-testid="config-panel" className="space-y-4 p-4">
      <h2 className="text-sm font-semibold text-slate-800">配置编辑</h2>
      {schema.length === 0 ? (
        <pre data-testid="config-raw" className="overflow-auto rounded bg-slate-50 p-3 text-xs">
          {JSON.stringify(values, null, 2)}
        </pre>
      ) : (
        <div className="space-y-3">
          {schema.map((field) => (
            <div key={field.key}>
              <label className="mb-1 block text-xs font-medium text-slate-600">{field.label}</label>
              {field.type === 'boolean' ? (
                <input
                  type="checkbox"
                  data-testid={`config-field-${field.key}`}
                  title={field.label}
                  checked={Boolean(values[field.key])}
                  onChange={(e) => handleChange(field.key, e.target.checked)}
                />
              ) : (
                <input
                  type={field.type === 'number' ? 'number' : 'text'}
                  data-testid={`config-field-${field.key}`}
                  title={field.label}
                  className="w-full rounded border border-slate-200 px-2 py-1 text-sm"
                  value={String(values[field.key] ?? '')}
                  onChange={(e) => handleChange(field.key, field.type === 'number' ? Number(e.target.value) : e.target.value)}
                />
              )}
            </div>
          ))}
        </div>
      )}
      <div className="flex gap-2">
        <button
          type="button"
          data-testid="config-save"
          className="inline-flex items-center gap-1 rounded bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-700 disabled:opacity-50"
          onClick={handleSave}
          disabled={!dirty}
        >
          <Save className="h-3.5 w-3.5" />
          保存
        </button>
        <button
          type="button"
          data-testid="config-reset"
          className="inline-flex items-center gap-1 rounded border border-slate-200 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
          onClick={handleReset}
        >
          <RotateCcw className="h-3.5 w-3.5" />
          重置
        </button>
      </div>
    </div>
  );
}
