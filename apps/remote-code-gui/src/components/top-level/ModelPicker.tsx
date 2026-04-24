import React, { useState } from 'react';
import { ChevronDown, Check } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface ModelOption {
  value: string;
  label: string;
  description?: string;
}

type Props = {
  models: ModelOption[];
  currentModel: string | null;
  onSelect: (model: string | null) => void;
  onCancel?: () => void;
};

export function ModelPicker({
  models,
  currentModel,
  onSelect,
  onCancel,
}: Props): React.ReactElement {
  const [focusedValue, setFocusedValue] = useState(currentModel ?? models[0]?.value ?? '');

  return (
    <div
      data-testid="model-picker"
      className="rounded-lg border border-gray-200 bg-white shadow-lg dark:border-gray-700 dark:bg-gray-800"
    >
      <div className="border-b border-gray-200 px-4 py-3 dark:border-gray-700">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          Select Model
        </h3>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Choose the AI model for this session
        </p>
      </div>

      <div className="max-h-80 overflow-y-auto p-2">
        {models.map((model) => (
          <button
            key={model.value}
            data-testid={`model-option-${model.value}`}
            className={cn(
              'flex w-full items-center justify-between rounded-md px-3 py-2 text-left transition-colors',
              focusedValue === model.value
                ? 'bg-cyan-50 dark:bg-cyan-900/20'
                : 'hover:bg-gray-50 dark:hover:bg-gray-700/50',
            )}
            onClick={() => {
              setFocusedValue(model.value);
              onSelect(model.value);
            }}
            onMouseEnter={() => setFocusedValue(model.value)}
          >
            <div className="flex flex-col">
              <span className="text-sm font-medium text-gray-900 dark:text-gray-100">
                {model.label}
              </span>
              {model.description && (
                <span className="text-xs text-gray-500 dark:text-gray-400">
                  {model.description}
                </span>
              )}
            </div>
            {currentModel === model.value && (
              <Check className="h-4 w-4 text-cyan-500" />
            )}
          </button>
        ))}
      </div>

      {onCancel && (
        <div className="border-t border-gray-200 p-3 dark:border-gray-700">
          <button
            data-testid="model-picker-cancel"
            onClick={onCancel}
            className="text-sm text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
          >
            Cancel
          </button>
        </div>
      )}
    </div>
  );
}
