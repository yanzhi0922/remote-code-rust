import { Shield } from 'lucide-react';

export interface SandboxPromptFooterHintProps {
  sandboxEnabled: boolean;
  sandboxType?: string;
}

export function SandboxPromptFooterHint({ sandboxEnabled, sandboxType }: SandboxPromptFooterHintProps) {
  if (!sandboxEnabled) return null;

  return (
    <div data-testid="sandbox-prompt-footer-hint" className="flex items-center gap-1.5 text-xs text-slate-400">
      <Shield className="h-3.5 w-3.5" />
      <span>沙箱已启用{sandboxType ? ` (${sandboxType})` : ''}</span>
    </div>
  );
}
