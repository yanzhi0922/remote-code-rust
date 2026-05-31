import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { MessageSquarePlus, Sparkles } from 'lucide-react';

interface FollowUpSuggestionsProps {
  lastAssistantText: string;
  hadToolCalls: boolean;
  onSuggestionClick: (text: string) => void;
}

function extractSuggestions(text: string, hadToolCalls: boolean, t: (key: string) => string): string[] {
  const suggestions: string[] = [];

  const questions = text.match(/(?:^|\n)\s*(?:\d+[\.\)]\s*)?([^\n?]+\?)/g);
  if (questions) {
    for (const q of questions.slice(0, 2)) {
      const cleaned = q.replace(/^\s*(?:\d+[\.\)]\s*)?/, '').trim();
      if (cleaned.length > 5 && cleaned.length < 120) suggestions.push(cleaned);
    }
  }

  const actionPatterns = [/\b(?:should|could|would|might|can)\b/i, /\b(?:need to|try to|want to)\b/i, /\b(?:next step|follow.up)\b/i];
  const lines = text.split('\n').filter((l) => l.trim().startsWith('-') || l.trim().startsWith('•') || l.trim().match(/^\d+[.)]/));
  for (const line of lines) {
    const content = line.replace(/^\s*(?:[-•]|\d+[.)])\s*/, '').trim();
    if (content.length > 10 && content.length < 100 && actionPatterns.some((p) => p.test(content))) {
      suggestions.push(content);
    }
  }

  if (hadToolCalls && suggestions.length < 3) {
    suggestions.push(t('followUp.reviewChanges'));
  }
  if (suggestions.length < 3) {
    suggestions.push(t('followUp.tellMeMore'));
  }

  return suggestions.slice(0, 3);
}

export function FollowUpSuggestions({ lastAssistantText, hadToolCalls, onSuggestionClick }: FollowUpSuggestionsProps) {
  const { t } = useTranslation();

  const suggestions = useMemo(
    () => extractSuggestions(lastAssistantText, hadToolCalls, t),
    [lastAssistantText, hadToolCalls, t],
  );

  if (!lastAssistantText || suggestions.length === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-2 border-b border-rc-border-secondary px-4 py-3 animate-fade-in">
      <Sparkles size={13} className="shrink-0 text-rc-accent-primary" />
      {suggestions.map((suggestion) => (
        <button
          key={suggestion}
          type="button"
          onClick={() => onSuggestionClick(suggestion)}
          className="flex items-center gap-1.5 rounded-full border border-rc-border-primary bg-rc-bg-elevated px-3 py-1.5 text-xs text-rc-text-secondary shadow-xs transition-colors hover:border-rc-accent-primary hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          <MessageSquarePlus size={11} className="shrink-0 text-rc-text-tertiary" />
          <span className="truncate max-w-[200px]">{suggestion}</span>
        </button>
      ))}
    </div>
  );
}
