import React from 'react';
import { Zap } from 'lucide-react';
import { cn } from '../../lib/utils';

type Props = {
  cooldown?: boolean;
};

export function FastIcon({ cooldown = false }: Props): React.ReactElement {
  return (
    <Zap
      data-testid="fast-icon"
      className={cn(
        'h-4 w-4',
        cooldown
          ? 'text-gray-400 dark:text-gray-500'
          : 'text-cyan-500',
      )}
    />
  );
}
