import { useState, useEffect, useRef } from 'react';
import { Code, X, Check, Ban } from 'lucide-react';

export interface LspRecommendationMenuProps {
  pluginName: string;
  pluginDescription?: string;
  fileExtension: string;
  onResponse: (response: 'yes' | 'no' | 'never' | 'disable') => void;
}

const AUTO_DISMISS_MS = 30_000;

export function LspRecommendationMenu({
  pluginName,
  pluginDescription,
  fileExtension,
  onResponse,
}: LspRecommendationMenuProps) {
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

  function handleSelect(value: 'yes' | 'no' | 'never' | 'disable') {
    onResponse(value);
    setVisible(false);
  }

  return (
    <div data-testid="lsp-recommendation-menu" className="rounded-lg border border-amber-200 bg-amber-50 p-4">
      <div className="mb-3 flex items-start justify-between">
        <div className="flex items-center gap-2">
          <Code className="h-5 w-5 text-amber-600" />
          <h3 className="text-sm font-semibold text-amber-900">LSP 插件推荐</h3>
        </div>
        <button
          type="button"
          data-testid="lsp-recommendation-dismiss"
          className="rounded p-1 hover:bg-amber-100"
          onClick={() => handleSelect('no')}
          title="关闭"
        >
          <X className="h-4 w-4 text-amber-400" />
        </button>
      </div>
      <p className="mb-2 text-sm text-amber-700">
        LSP 提供代码智能功能，如跳转到定义和错误检查。
      </p>
      <div className="mb-3 space-y-1 text-sm text-amber-800">
        <p>插件: <strong>{pluginName}</strong></p>
        {pluginDescription && (
          <p className="text-amber-600">{pluginDescription}</p>
        )}
        <p>触发文件: <strong>{fileExtension}</strong></p>
      </div>
      <p className="mb-3 text-sm text-amber-700">是否安装此 LSP 插件？</p>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          data-testid="lsp-recommendation-yes"
          className="inline-flex items-center gap-1 rounded bg-amber-600 px-3 py-1.5 text-sm text-white hover:bg-amber-700"
          onClick={() => handleSelect('yes')}
        >
          <Check className="h-3.5 w-3.5" />
          安装 {pluginName}
        </button>
        <button
          type="button"
          data-testid="lsp-recommendation-no"
          className="rounded border border-amber-300 px-3 py-1.5 text-sm text-amber-700 hover:bg-amber-100"
          onClick={() => handleSelect('no')}
        >
          暂不
        </button>
        <button
          type="button"
          data-testid="lsp-recommendation-never"
          className="rounded px-3 py-1.5 text-sm text-amber-600 hover:bg-amber-100"
          onClick={() => handleSelect('never')}
        >
          永不提示 {pluginName}
        </button>
        <button
          type="button"
          data-testid="lsp-recommendation-disable"
          className="inline-flex items-center gap-1 rounded px-3 py-1.5 text-sm text-amber-600 hover:bg-amber-100"
          onClick={() => handleSelect('disable')}
        >
          <Ban className="h-3.5 w-3.5" />
          禁用所有LSP推荐
        </button>
      </div>
    </div>
  );
}
