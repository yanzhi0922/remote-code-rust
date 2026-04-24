import { useCallback, useRef, useState } from 'react';
import type { TranscriptShareResponse } from './TranscriptSharePrompt';
import type { FeedbackSurveyResponse, SurveyState } from './utils';

type UseSurveyStateOptions = {
  hideThanksAfterMs: number;
  onOpen: (appearanceId: string) => void | Promise<void>;
  onSelect: (
    appearanceId: string,
    selected: FeedbackSurveyResponse,
  ) => void | Promise<void>;
  shouldShowTranscriptPrompt?: (selected: FeedbackSurveyResponse) => boolean;
  onTranscriptPromptShown?: (
    appearanceId: string,
    surveyResponse: FeedbackSurveyResponse,
  ) => void;
  onTranscriptSelect?: (
    appearanceId: string,
    selected: TranscriptShareResponse,
    surveyResponse: FeedbackSurveyResponse | null,
  ) => boolean | Promise<boolean>;
};

export function useSurveyState({
  hideThanksAfterMs,
  onOpen,
  onSelect,
  shouldShowTranscriptPrompt,
  onTranscriptPromptShown,
  onTranscriptSelect,
}: UseSurveyStateOptions): {
  state: SurveyState;
  lastResponse: FeedbackSurveyResponse | null;
  open: () => void;
  handleSelect: (selected: FeedbackSurveyResponse) => boolean;
  handleTranscriptSelect: (selected: TranscriptShareResponse) => void;
} {
  const [state, setState] = useState<SurveyState>('closed');
  const [lastResponse, setLastResponse] = useState<FeedbackSurveyResponse | null>(null);
  const appearanceId = useRef(crypto.randomUUID());
  const lastResponseRef = useRef<FeedbackSurveyResponse | null>(null);

  const showThanksThenClose = useCallback(() => {
    setState('thanks');
    setTimeout(() => {
      setState('closed');
      setLastResponse(null);
    }, hideThanksAfterMs);
  }, [hideThanksAfterMs]);

  const showSubmittedThenClose = useCallback(() => {
    setState('submitted');
    setTimeout(() => setState('closed'), hideThanksAfterMs);
  }, [hideThanksAfterMs]);

  const open = useCallback(() => {
    if (state !== 'closed') {
      return;
    }
    setState('open');
    appearanceId.current = crypto.randomUUID();
    void onOpen(appearanceId.current);
  }, [state, onOpen]);

  const handleSelect = useCallback(
    (selected: FeedbackSurveyResponse): boolean => {
      setLastResponse(selected);
      lastResponseRef.current = selected;
      void onSelect(appearanceId.current, selected);
      if (selected === 'dismissed') {
        setState('closed');
        setLastResponse(null);
      } else if (shouldShowTranscriptPrompt?.(selected)) {
        setState('transcript_prompt');
        onTranscriptPromptShown?.(appearanceId.current, selected);
        return true;
      } else {
        showThanksThenClose();
      }
      return false;
    },
    [showThanksThenClose, onSelect, shouldShowTranscriptPrompt, onTranscriptPromptShown],
  );

  const handleTranscriptSelect = useCallback(
    (selected: TranscriptShareResponse) => {
      switch (selected) {
        case 'yes':
          setState('submitting');
          void (async () => {
            try {
              const success = await onTranscriptSelect?.(
                appearanceId.current,
                selected,
                lastResponseRef.current,
              );
              if (success) {
                showSubmittedThenClose();
              } else {
                showThanksThenClose();
              }
            } catch {
              showThanksThenClose();
            }
          })();
          break;
        case 'no':
        case 'dont_ask_again':
          void onTranscriptSelect?.(
            appearanceId.current,
            selected,
            lastResponseRef.current,
          );
          showThanksThenClose();
          break;
      }
    },
    [showThanksThenClose, showSubmittedThenClose, onTranscriptSelect],
  );

  return {
    state,
    lastResponse,
    open,
    handleSelect,
    handleTranscriptSelect,
  };
}
