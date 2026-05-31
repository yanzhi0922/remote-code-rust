import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff } from 'lucide-react';

export interface SettingInputProps {
  label: string;
  value: string | number;
  onChange: (value: string) => void;
  type?: 'text' | 'number' | 'password';
  description?: string;
  placeholder?: string;
}

export function SettingInput({
  label,
  value,
  onChange,
  type = 'text',
  description,
  placeholder,
}: SettingInputProps) {
  const { t } = useTranslation();
  const [showPassword, setShowPassword] = useState(false);

  const isPassword = type === 'password';
  const inputType = isPassword ? (showPassword ? 'text' : 'password') : type;

  return (
    <div className="space-y-1.5">
      <label className="block text-sm font-medium text-rc-text-primary">{label}</label>
      <div className="relative">
        <input
          type={inputType}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary placeholder:text-rc-text-tertiary focus:border-rc-border-focus focus:outline-none"
          data-testid="setting-input"
        />
        {isPassword && (
          <button
            type="button"
            onClick={() => setShowPassword((prev) => !prev)}
            className="absolute right-2 top-1/2 -translate-y-1/2 text-rc-text-tertiary hover:text-rc-text-primary"
            aria-label={showPassword ? t('settingInput.hidePassword') : t('settingInput.showPassword')}
            data-testid="toggle-password"
          >
            {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
          </button>
        )}
      </div>
      {description && <p className="text-xs leading-5 text-rc-text-tertiary">{description}</p>}
    </div>
  );
}
