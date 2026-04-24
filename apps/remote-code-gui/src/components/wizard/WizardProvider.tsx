import React, { createContext, useCallback, useContext, useState } from 'react';

export interface WizardContextValue<T = Record<string, unknown>> {
  currentStepIndex: number;
  totalSteps: number;
  title?: string;
  showStepCounter: boolean;
  wizardData: T;
  goNext: () => void;
  goBack: () => void;
  setWizardData: React.Dispatch<React.SetStateAction<T>>;
}

export const WizardContext = createContext<WizardContextValue | null>(null);

export function useWizard<T = Record<string, unknown>>(): WizardContextValue<T> {
  const ctx = useContext(WizardContext);
  if (!ctx) {
    throw new Error('useWizard must be used within a WizardProvider');
  }
  return ctx as WizardContextValue<T>;
}

export interface WizardProviderProps<T = Record<string, unknown>> {
  steps: React.ReactElement[];
  initialData?: T;
  onComplete: (data: T) => void;
  onCancel?: () => void;
  children: React.ReactNode;
  title?: string;
  showStepCounter?: boolean;
}

export function WizardProvider<T = Record<string, unknown>>({
  steps,
  initialData,
  onComplete,
  onCancel,
  children,
  title,
  showStepCounter = true,
}: WizardProviderProps<T>): React.ReactElement {
  const [currentStepIndex, setCurrentStepIndex] = useState(0);
  const [wizardData, setWizardData] = useState<T>(initialData ?? ({} as T));
  const [isCompleted, setIsCompleted] = useState(false);

  const goNext = useCallback(() => {
    if (currentStepIndex < steps.length - 1) {
      setCurrentStepIndex((prev) => prev + 1);
    } else {
      setIsCompleted(true);
    }
  }, [currentStepIndex, steps.length]);

  const goBack = useCallback(() => {
    if (currentStepIndex > 0) {
      setCurrentStepIndex((prev) => prev - 1);
    } else {
      onCancel?.();
    }
  }, [currentStepIndex, onCancel]);

  // Call onComplete when wizard is completed
  React.useEffect(() => {
    if (isCompleted) {
      onComplete(wizardData);
    }
  }, [isCompleted, wizardData, onComplete]);

  const value: WizardContextValue<T> = {
    currentStepIndex,
    totalSteps: steps.length,
    title,
    showStepCounter,
    wizardData,
    goNext,
    goBack,
    setWizardData,
  };

  return (
    <WizardContext.Provider value={value as WizardContextValue}>
      {children}
    </WizardContext.Provider>
  );
}
