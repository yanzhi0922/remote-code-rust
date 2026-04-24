/**
 * Select — 下拉选择组件。
 *
 * 支持搜索过滤、分组显示和选中项高亮。
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { ChevronDown } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface SelectOption {
  value: string;
  label: string;
  group?: string;
}

export interface SelectProps {
  options: SelectOption[];
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  searchable?: boolean;
  className?: string;
}

export function Select({
  options,
  value,
  onChange,
  placeholder = 'Select...',
  searchable = false,
  className,
}: SelectProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const containerRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  const selectedOption = options.find((o) => o.value === value);

  const filtered = searchable && search
    ? options.filter((o) =>
        o.label.toLowerCase().includes(search.toLowerCase()),
      )
    : options;

  // Group options
  const grouped: Record<string, SelectOption[]> = {};
  for (const opt of filtered) {
    const group = opt.group ?? '';
    if (!grouped[group]) grouped[group] = [];
    grouped[group].push(opt);
  }

  const handleSelect = useCallback(
    (val: string) => {
      onChange(val);
      setOpen(false);
      setSearch('');
    },
    [onChange],
  );

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      ) {
        setOpen(false);
        setSearch('');
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  // Focus search input when opened
  useEffect(() => {
    if (open && searchable) {
      searchInputRef.current?.focus();
    }
  }, [open, searchable]);

  return (
    <div
      ref={containerRef}
      className={cn('relative', className)}
      data-testid="select"
    >
      {/* Trigger */}
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        className={cn(
          'flex h-10 w-full items-center justify-between rounded-xl border border-slate-300 bg-white px-3 text-sm',
          'hover:border-slate-400 focus:outline-none focus:ring-2 focus:ring-slate-400 focus:ring-offset-1',
        )}
        data-testid="select-trigger"
      >
        <span className={selectedOption ? 'text-slate-900' : 'text-slate-400'}>
          {selectedOption?.label ?? placeholder}
        </span>
        <ChevronDown
          className={cn(
            'h-4 w-4 text-slate-400 transition-transform',
            open && 'rotate-180',
          )}
        />
      </button>

      {/* Dropdown */}
      {open && (
        <div
          className="absolute left-0 right-0 top-full z-50 mt-1 overflow-hidden rounded-xl border border-slate-200 bg-white shadow-lg"
          data-testid="select-dropdown"
        >
          {/* Search input */}
          {searchable && (
            <div className="border-b border-slate-100 px-3 py-2">
              <input
                ref={searchInputRef}
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search..."
                className="w-full text-sm outline-none"
                data-testid="select-search"
              />
            </div>
          )}

          {/* Options */}
          <div className="max-h-60 overflow-y-auto py-1">
            {Object.entries(grouped).map(([group, opts]) => (
              <div key={group}>
                {group && (
                  <div
                    className="px-3 py-1.5 text-xs font-semibold uppercase text-slate-400"
                    data-testid={`select-group-${group}`}
                  >
                    {group}
                  </div>
                )}
                {opts.map((opt) => (
                  <button
                    key={opt.value}
                    type="button"
                    onClick={() => handleSelect(opt.value)}
                    className={cn(
                      'flex w-full items-center px-3 py-2 text-left text-sm transition-colors',
                      opt.value === value
                        ? 'bg-slate-100 text-slate-900 font-medium'
                        : 'text-slate-700 hover:bg-slate-50',
                    )}
                    data-testid={`select-option-${opt.value}`}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            ))}
            {filtered.length === 0 && (
              <div
                className="px-3 py-4 text-center text-sm text-slate-400"
                data-testid="select-empty"
              >
                No results
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
