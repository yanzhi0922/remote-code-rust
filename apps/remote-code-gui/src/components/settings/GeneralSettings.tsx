import type { FullSettings } from '../../lib/types';
import { SettingInput } from './SettingInput';
import { ToggleSwitch } from './ToggleSwitch';

export interface GeneralSettingsProps {
  settings: FullSettings;
  onUpdate: (updates: Record<string, unknown>) => void;
}

export function GeneralSettings({ settings, onUpdate }: GeneralSettingsProps) {
  return (
    <div className="space-y-6" data-testid="general-settings">
      <h3 className="text-lg font-semibold text-slate-800">通用设置</h3>

      <ToggleSwitch
        label="Verbose 模式"
        description="启用详细日志输出，便于调试问题"
        checked={settings.verbose}
        onChange={(checked) => onUpdate({ verbose: checked })}
      />

      <SettingInput
        label="超时时间 (ms)"
        description="请求超时时间，单位毫秒"
        value={settings.timeout_ms}
        type="number"
        onChange={(value) => onUpdate({ timeout_ms: Number(value) })}
        placeholder="30000"
      />

      <SettingInput
        label="最大重试次数"
        description="请求失败后的最大重试次数"
        value={settings.max_retries}
        type="number"
        onChange={(value) => onUpdate({ max_retries: Number(value) })}
        placeholder="3"
      />

      <SettingInput
        label="重试初始退避 (ms)"
        description="首次重试前的等待时间"
        value={settings.retry_initial_backoff_ms}
        type="number"
        onChange={(value) => onUpdate({ retry_initial_backoff_ms: Number(value) })}
        placeholder="1000"
      />

      <SettingInput
        label="重试最大退避 (ms)"
        description="重试退避的上限时间"
        value={settings.retry_max_backoff_ms}
        type="number"
        onChange={(value) => onUpdate({ retry_max_backoff_ms: Number(value) })}
        placeholder="30000"
      />

      <ToggleSwitch
        label="遵守 Retry-After"
        description="是否遵守服务端返回的 Retry-After 头"
        checked={settings.respect_retry_after}
        onChange={(checked) => onUpdate({ respect_retry_after: checked })}
      />
    </div>
  );
}
