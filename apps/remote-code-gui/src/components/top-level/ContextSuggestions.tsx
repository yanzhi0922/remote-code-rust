import { Lightbulb } from 'lucide-react';

export interface ContextSuggestion {
  id: string;
  text: string;
  type: 'file' | 'command' | 'tip';
}

export interface ContextSuggestionsProps {
  suggestions: ContextSuggestion[];
  onSelect?: (suggestion: ContextSuggestion) => void;
}

export function ContextSuggestions({ suggestions, onSelect }: ContextSuggestionsProps) {
  if (suggestions.length === 0) return null;

  return (
    <div data-testid="context-suggestions" className="space-y-1">
      <div className="flex items-center gap-1 text-xs text-slate-400">
        <Lightbulb className="h-3 w-3" />
        <span>建议</span>
      </div>
      <div className="flex flex-wrap gap-1">
        {suggestions.map((suggestion) => (
          <button
            key={suggestion.id}
            type="button"
            data-testid={`context-suggestion-${suggestion.id}`}
            className="rounded-full border border-slate-200 bg-slate-50 px-2.5 py-1 text-xs text-slate-600 hover:bg-slate-100"
            onClick={() => onSelect?.(suggestion)}
          >
            {suggestion.text}
          </button>
        ))}
      </div>
    </div>
  );
}
