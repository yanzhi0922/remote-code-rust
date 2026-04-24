import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { VoiceIndicator } from './VoiceIndicator';

describe('VoiceIndicator', () => {
  afterEach(cleanup);

  it('isSupported=false 时返回 null', () => {
    render(
      <VoiceIndicator isListening={false} isSupported={false} onToggle={vi.fn()} />,
    );
    expect(screen.queryByTestId('voice-indicator')).not.toBeInTheDocument();
  });

  it('isSupported=true 时渲染并显示 data-testid', () => {
    render(
      <VoiceIndicator isListening={false} isSupported onToggle={vi.fn()} />,
    );
    expect(screen.getByTestId('voice-indicator')).toBeInTheDocument();
  });

  it('未监听时显示 MicOff 图标', () => {
    render(
      <VoiceIndicator isListening={false} isSupported onToggle={vi.fn()} />,
    );
    expect(screen.getByLabelText('开始语音输入')).toBeInTheDocument();
  });

  it('监听时显示 Mic 图标', () => {
    render(
      <VoiceIndicator isListening isSupported onToggle={vi.fn()} />,
    );
    expect(screen.getByLabelText('停止语音输入')).toBeInTheDocument();
  });

  it('监听时显示红色样式', () => {
    render(
      <VoiceIndicator isListening isSupported onToggle={vi.fn()} />,
    );
    const button = screen.getByTestId('voice-indicator');
    expect(button.className).toContain('text-red-500');
  });

  it('点击触发 onToggle', () => {
    const onToggle = vi.fn();
    render(
      <VoiceIndicator isListening={false} isSupported onToggle={onToggle} />,
    );
    fireEvent.click(screen.getByTestId('voice-indicator'));
    expect(onToggle).toHaveBeenCalled();
  });
});
