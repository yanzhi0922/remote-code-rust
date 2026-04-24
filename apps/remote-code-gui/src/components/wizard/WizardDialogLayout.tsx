import React, { type ReactNode } from 'react';
import { X } from 'lucide-react';
import { useWizard } from './WizardProvider';
import { WizardNavigationFooter } from './WizardNavigationFooter';

type Props = {
  title?: string;
  children: ReactNode;
  subtitle?: string;
  footerText?: ReactNode;
};

export function WizardDialogLayout({
  title: titleOverride,
  children,
  subtitle,
  footerText,
}: Props): React.ReactElement {
  const { currentStepIndex, totalSteps, title: providerTitle, showStepCounter, goBack } = useWizard();

  const title = titleOverride || providerTitle || 'Wizard';
  const stepSuffix = showStepCounter !== false ? ` (${currentStepIndex + 1}/${totalSteps})` : '';

  return (
    <>
      <div
        data-testid="wizard-dialog-layout"
        className="rounded-lg border border-gray-200 bg-white shadow-lg dark:border-gray-700 dark:bg-gray-800"
      >
        <div className="flex items-center justify-between border-b border-gray-200 px-4 py-3 dark:border-gray-700">
          <div>
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              {title}{stepSuffix}
            </h3>
            {subtitle && (
              <p className="text-sm text-gray-500 dark:text-gray-400">{subtitle}</p>
            )}
          </div>
          <button
            data-testid="wizard-cancel-btn"
            aria-label="Cancel"
            onClick={goBack}
            className="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
          >
            <X className="h-5 w-5" />
          </button>
        </div>
        <div className="p-4">{children}</div>
      </div>
      <WizardNavigationFooter instructions={footerText} />
    </>
  );
}
