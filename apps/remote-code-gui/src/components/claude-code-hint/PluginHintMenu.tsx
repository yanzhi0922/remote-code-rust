import { useState, useEffect, useRef } from 'react';
import { Puzzle, X, Check, Ban } from 'lucide-react';

export interface PluginHintMenuProps {
  pluginName: string;
  pluginDescription?: string;
  marketplaceName: string;
  sourceCommand: string;
  onResponse: (response: 'yes' | 'no' | 'disable') => void;
}

const AUTO_DISMISS_MS = 30_000;

export function PluginHintMenu({
  pluginName,
  pluginDescription,
  marketplaceName,
  sourceCommand,
  onResponse,
}: PluginHintMenuProps) {
  const [visible, setVisible] = useState(true);
  const onResponseRef = useRef(onResponse);
  onResponseRef.current = onResponse;

  useEffect(() => {
    const timeoutId = setTimeout(() => {
      onResponseRef.current('no');
      setVisible(false);
    }, AUTO_DISMISS_MS);
    return () => clearTimeout(timeoutId);
  }, []);

  if (!visible) return null;

  function handleSelect(value: 'yes' | 'no' | 'disable') {
    onResponse(value);
    setVisible(false);
  }

  return (
    <div data-testid="plugin-hint-menu" className="rounded-lg border border-blue-200 bg-blue-50 p-4">
      <div className="mb-3 flex items-start justify-between">
        <div className="flex items-center gap-2">
          <Puzzle className="h-5 w-5 text-blue-600" />
          <h3 className="text-sm font-semibold text-blue-900">插件推荐</h3>
        </div>
        <button
          type="button"
          data-testid="plugin-hint-dismiss"
          className="rounded p-1 hover:bg-blue-100"
          onClick={() => handleSelect('no')}
          title="关闭"
        >
          <X className="h-4 w-4 text-blue-400" />
        </button>
      </div>
      <p className="mb-2 text-sm text-blue-700">
        <code className="font-semibold">{sourceCommand}</code> 命令建议安装插件。
      </p>
      <div className="mb-3 space-y-1 text-sm text-blue-800">
        <p>插件: <strong>{pluginName}</strong></p>
        <p>市场: <strong>{marketplaceName}</strong></p>
        {pluginDescription && (
          <p className="text-blue-600">{pluginDescription}</p>
        )}
      </div>
      <p className="mb-3 text-sm text-blue-700">是否安装？</p>
      <div className="flex gap-2">
        <button
          type="button"
          data-testid="plugin-hint-yes"
          className="inline-flex items-center gap-1 rounded bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-700"
          onClick={() => handleSelect('yes')}
        >
          <Check className="h-3.5 w-3.5" />
          安装 {pluginName}
        </button>
        <button
          type="button"
          data-testid="plugin-hint-no"
          className="rounded border border-blue-300 px-3 py-1.5 text-sm text-blue-700 hover:bg-blue-100"
          onClick={() => handleSelect('no')}
        >
          不安装
        </button>
        <button
          type="button"
          data-testid="plugin-hint-disable"
          className="inline-flex items-center gap-1 rounded px-3 py-1.5 text-sm text-blue-600 hover:bg-blue-100"
          onClick={() => handleSelect('disable')}
        >
          <Ban className="h-3.5 w-3.5" />
          不再提示
        </button>
      </div>
    </div>
  );
}
