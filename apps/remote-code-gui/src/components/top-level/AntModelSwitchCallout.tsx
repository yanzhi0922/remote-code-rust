import { Zap, X } from 'lucide-react';

export interface AntModelSwitchCalloutProps {
  fromModel: string;
  toModel: string;
  onDismiss: () => void;
}

export function AntModelSwitchCallout({ fromModel, toModel, onDismiss }: AntModelSwitchCalloutProps) {
  return (
    <div data-testid="ant-model-switch-callout" className="flex items-center gap-2 rounded-lg border border-blue-200 bg-blue-50 px-3 py-2">
      <Zap className="h-4 w-4 shrink-0 text-blue-600" />
      <span className="text-sm text-blue-700">
        模型已从 <strong>{fromModel}</strong> 切换到 <strong>{toModel}</strong>
      </span>
      <button
        type="button"
        data-testid="ant-model-switch-dismiss"
        className="ml-auto rounded p-0.5 hover:bg-blue-100"
        onClick={onDismiss}
        title="关闭"
      >
        <X className="h-3.5 w-3.5 text-blue-400" />
      </button>
    </div>
  );
}
