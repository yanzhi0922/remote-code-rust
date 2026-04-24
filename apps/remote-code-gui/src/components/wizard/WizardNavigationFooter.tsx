import React, { type ReactNode } from 'react';

type Props = {
  instructions?: ReactNode;
};

export function WizardNavigationFooter({
  instructions,
}: Props): React.ReactElement {
  const defaultInstructions = (
    <span>
      <span className="text-cyan-500">↑↓</span> navigate ·{' '}
      <span className="text-cyan-500">Enter</span> select ·{' '}
      <span className="text-cyan-500">Esc</span> go back
    </span>
  );

  return (
    <div data-testid="wizard-navigation-footer" className="ml-3 mt-2">
      <span className="text-sm text-gray-500 dark:text-gray-400">
        {instructions ?? defaultInstructions}
      </span>
    </div>
  );
}
