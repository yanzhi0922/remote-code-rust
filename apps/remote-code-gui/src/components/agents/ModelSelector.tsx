import { useMemo, useState } from 'react';
import { Cpu, ChevronDown } from 'lucide-react';
import type { ModelOption } from './AgentFormData';

export interface ModelSelectorProps {
  value: string | null;
  onChange: (model: string | null) => void;
  models: ModelOption[];
}

export function ModelSelector({ value, onChange, models }: ModelSelectorProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');

  const grouped = useMemo(() => {
    const filtered = models.filter(
      (m) =>
        m.name.toLowerCase().includes(search.toLowerCase()) ||
        m.id.toLowerCase().includes(search.toLowerCase()) ||
        m.provider.toLowerCase().includes(search.toLowerCase()),
    );
    const map = new Map<string, ModelOption[]>();
    for (const m of filtered) {
      const list = map.get(m.provider) || [];
      list.push(m);
      map.set(m.provider, list);
    }
    return map;
  }, [models, search]);

  const selectedLabel = useMemo(() => {
    if (value === null) return '使用默认模型';
    const found = models.find((m) => m.id === value);
    return found ? `${found.name} (${found.id})` : value;
  }, [value, models]);

  function handleSelect(modelId: string | null) {
    onChange(modelId);
    setOpen(false);
    setSearch('');
  }

  return (
    <div className="relative" data-testid="model-selector">
      <label className="mb-1 flex items-center gap-1.5 text-sm font-medium text-slate-700">
        <Cpu className="h-4 w-4" />
        模型
      </label>

      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex w-full items-center justify-between rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm hover:border-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
      >
        <span className="truncate">{selectedLabel}</span>
        <ChevronDown className={`h-4 w-4 text-slate-400 transition-transform ${open ? 'rotate-180' : ''}`} />
      </button>

      {open && (
        <div className="absolute z-10 mt-1 w-full rounded-xl border border-slate-200 bg-white py-1 shadow-lg">
          <div className="border-b border-slate-100 px-3 py-2">
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="搜索模型..."
              aria-label="搜索模型"
              className="w-full rounded-lg border border-slate-200 px-2 py-1 text-sm focus:border-blue-500 focus:outline-none"
              autoFocus
            />
          </div>

          <div className="max-h-48 overflow-y-auto">
            <button
              type="button"
              onClick={() => handleSelect(null)}
              className={`w-full px-3 py-2 text-left text-sm hover:bg-slate-50 ${
                value === null ? 'bg-blue-50 font-medium text-blue-700' : 'text-slate-700'
              }`}
            >
              使用默认模型
            </button>

            {Array.from(grouped.entries()).map(([provider, providerModels]) => (
              <div key={provider}>
                <div className="px-3 py-1 text-xs font-semibold uppercase text-slate-400">
                  {provider}
                </div>
                {providerModels.map((model) => (
                  <button
                    key={model.id}
                    type="button"
                    onClick={() => handleSelect(model.id)}
                    className={`w-full px-3 py-2 text-left text-sm hover:bg-slate-50 ${
                      value === model.id ? 'bg-blue-50 font-medium text-blue-700' : 'text-slate-700'
                    }`}
                  >
                    <span>{model.name}</span>
                    <span className="ml-1 text-xs text-slate-400">({model.id})</span>
                  </button>
                ))}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
