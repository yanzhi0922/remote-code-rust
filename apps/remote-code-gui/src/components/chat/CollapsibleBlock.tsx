import { ChevronRight } from 'lucide-react';
import { useState } from 'react';
import { cn } from '../../lib/utils';

interface CollapsibleBlockProps {
  summary: React.ReactNode;
  children: React.ReactNode;
  defaultOpen?: boolean;
  buttonLabel?: string;
  iconColor?: string;
  className?: string;
  contentClassName?: string;
}

export default function CollapsibleBlock({
  summary,
  children,
  defaultOpen = false,
  buttonLabel,
  iconColor = 'text-rc-text-tertiary',
  className,
  contentClassName,
}: CollapsibleBlockProps) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <div className={cn('overflow-hidden rounded-md border border-rc-border-primary bg-rc-bg-elevated', className)}>
      <button
        type="button"
        aria-expanded={isOpen}
        aria-label={buttonLabel}
        className="flex w-full items-center gap-2 px-3 py-1.5 text-left transition-colors hover:bg-rc-bg-hover"
        onClick={() => setIsOpen((state) => !state)}
      >
        <ChevronRight
          size={14}
          className={cn(iconColor, 'shrink-0 transition-transform', isOpen && 'rotate-90')}
        />
        <div className="min-w-0 flex-1">{summary}</div>
      </button>

      {/* CSS Grid collapse: GPU-accelerated 0fr↔1fr transition, no JS height measurement */}
      <div
        className="grid-collapse"
        data-collapsed={!isOpen}
      >
        <div className="grid-collapse-inner">
          <div className={cn('border-t border-rc-border-secondary px-3 pb-3 pt-2', contentClassName)}>
            {children}
          </div>
        </div>
      </div>
    </div>
  );
}
