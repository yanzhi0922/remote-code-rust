import { type ReactNode } from 'react';
import { DollarSign, ExternalLink } from 'lucide-react';

interface Props {
  onDone: () => void;
  threshold?: number;
  currentSpend?: number;
}

export function CostThresholdDialog({ onDone, threshold = 5, currentSpend }: Props): ReactNode {
  return (
    <div
      data-testid="cost-threshold-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center gap-2">
          <DollarSign className="h-5 w-5 text-yellow-500" />
          <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Cost Threshold Reached
          </h3>
        </div>

        <p className="mt-3 text-sm text-gray-600 dark:text-gray-400">
          {currentSpend != null
            ? `You've spent $${currentSpend.toFixed(2)} this session.`
            : `You've spent $${threshold} on the API this session.`}
        </p>

        <p className="mt-2 text-sm text-gray-600 dark:text-gray-400">
          Learn more about how to monitor your spending:
        </p>

        <a
          href="https://docs.anthropic.com/en/docs/about-claude/pricing"
          target="_blank"
          rel="noopener noreferrer"
          className="mt-1 inline-flex items-center gap-1 text-sm text-blue-600 hover:text-blue-800 dark:text-blue-400"
        >
          Cost documentation <ExternalLink className="h-3 w-3" />
        </a>

        <div className="mt-4 flex justify-end">
          <button
            data-testid="cost-dialog-done"
            onClick={onDone}
            className="rounded bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700"
          >
            Got it, thanks!
          </button>
        </div>
      </div>
    </div>
  );
}
