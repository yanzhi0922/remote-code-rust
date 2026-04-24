import { useState } from 'react';
import { Search, X } from 'lucide-react';

export interface SearchBoxProps {
  value?: string;
  placeholder?: string;
  onSearch?: (query: string) => void;
  onChange?: (query: string) => void;
}

export function SearchBox({ value: controlledValue, placeholder = '搜索...', onSearch, onChange }: SearchBoxProps) {
  const [internalValue, setInternalValue] = useState('');
  const value = controlledValue ?? internalValue;

  function handleChange(newValue: string) {
    setInternalValue(newValue);
    onChange?.(newValue);
  }

  function handleClear() {
    handleChange('');
    onSearch?.('');
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Enter') {
      onSearch?.(value);
    }
  }

  return (
    <div data-testid="search-box" className="inline-flex items-center gap-2 rounded-lg border border-slate-200 bg-white px-3 py-1.5">
      <Search className="h-4 w-4 text-slate-400" />
      <input
        data-testid="search-box-input"
        type="text"
        className="flex-1 bg-transparent text-sm outline-none"
        placeholder={placeholder}
        value={value}
        onChange={(e) => handleChange(e.target.value)}
        onKeyDown={handleKeyDown}
      />
      {value && (
        <button type="button" data-testid="search-box-clear" className="rounded p-0.5 hover:bg-slate-100" onClick={handleClear} title="清除">
          <X className="h-3.5 w-3.5 text-slate-400" />
        </button>
      )}
    </div>
  );
}
