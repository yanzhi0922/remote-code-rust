import { X } from 'lucide-react';
import { useState } from 'react';

interface ElicitationField {
  name: string;
  label: string;
  type: 'text' | 'password' | 'select';
  options?: string[];
}

interface McpElicitationDialogProps {
  visible: boolean;
  title: string;
  message: string;
  fields: ElicitationField[];
  onSubmit: (values: Record<string, string>) => void;
  onCancel: () => void;
}

export function McpElicitationDialog({
  visible,
  title,
  message,
  fields,
  onSubmit,
  onCancel,
}: McpElicitationDialogProps) {
  const [values, setValues] = useState<Record<string, string>>(() => {
    const init: Record<string, string> = {};
    for (const f of fields) {
      init[f.name] = f.type === 'select' && f.options?.length ? f.options[0] : '';
    }
    return init;
  });

  if (!visible) return null;

  function handleChange(name: string, value: string) {
    setValues((prev) => ({ ...prev, [name]: value }));
  }

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    onSubmit(values);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40" data-testid="mcp-elicitation-overlay">
      <div className="w-full max-w-md rounded-2xl border border-slate-200 bg-white p-6 shadow-xl" data-testid="mcp-elicitation-dialog">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold text-slate-800">{title}</h3>
          <button
            type="button"
            onClick={onCancel}
            className="rounded-lg p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
            data-testid="mcp-elicitation-close"
            aria-label="关闭"
          >
            <X size={16} />
          </button>
        </div>

        {message && (
          <p className="mt-2 text-sm text-slate-600">{message}</p>
        )}

        <form onSubmit={handleSubmit} className="mt-4 flex flex-col gap-3">
          {fields.map((field) => (
            <div key={field.name}>
              <label className="mb-1 block text-sm font-medium text-slate-700">{field.label}</label>
              {field.type === 'select' && field.options ? (
                <select
                  value={values[field.name] ?? ''}
                  onChange={(e) => handleChange(field.name, e.target.value)}
                  className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-300 focus:outline-none"
                  data-testid={`mcp-elicitation-field-${field.name}`}
                  aria-label={field.label}
                >
                  {field.options.map((opt) => (
                    <option key={opt} value={opt}>{opt}</option>
                  ))}
                </select>
              ) : (
                <input
                  type={field.type}
                  value={values[field.name] ?? ''}
                  onChange={(e) => handleChange(field.name, e.target.value)}
                  className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-300 focus:outline-none"
                  data-testid={`mcp-elicitation-field-${field.name}`}
                  aria-label={field.label}
                  placeholder={field.label}
                />
              )}
            </div>
          ))}

          <div className="mt-2 flex items-center justify-end gap-2">
            <button
              type="button"
              onClick={onCancel}
              className="rounded-xl border border-slate-200 px-4 py-2 text-sm text-slate-600 hover:bg-slate-50"
              data-testid="mcp-elicitation-cancel"
            >
              取消
            </button>
            <button
              type="submit"
              className="rounded-xl bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-700"
              data-testid="mcp-elicitation-submit"
            >
              提交
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
