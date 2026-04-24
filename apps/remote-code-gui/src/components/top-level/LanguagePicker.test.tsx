import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { LanguagePicker } from './LanguagePicker';

afterEach(() => {
  cleanup();
});

describe('LanguagePicker', () => {
  const languages = [
    { code: 'zh', name: '中文' },
    { code: 'en', name: 'English' },
  ];

  it('renders language options', () => {
    render(<LanguagePicker languages={languages} value="zh" onChange={() => {}} />);
    expect(screen.getByTestId('language-picker')).toBeInTheDocument();
    expect(screen.getByText('中文')).toBeInTheDocument();
    expect(screen.getByText('English')).toBeInTheDocument();
  });

  it('calls onChange', () => {
    const onChange = vi.fn();
    render(<LanguagePicker languages={languages} value="zh" onChange={onChange} />);
    fireEvent.change(screen.getByTestId('language-picker-select'), { target: { value: 'en' } });
    expect(onChange).toHaveBeenCalledWith('en');
  });
});
