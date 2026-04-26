import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import { StatusLine } from './StatusLine';

describe('StatusLine', () => {
  afterEach(() => { cleanup(); });

  it('renders provider, model, and permission mode', () => {
    const { getByTestId, getByText } = render(
      <StatusLine status={{ provider: 'OpenAI', model: 'gpt-4', permissionMode: 'auto' }} />,
    );
    expect(getByTestId('status-line')).toBeInTheDocument();
    expect(getByTestId('status-provider')).toBeInTheDocument();
    expect(getByText('OpenAI')).toBeInTheDocument();
    expect(getByTestId('status-model')).toBeInTheDocument();
    expect(getByText('gpt-4')).toBeInTheDocument();
    expect(getByTestId('status-permission')).toBeInTheDocument();
    expect(getByText('auto')).toBeInTheDocument();
  });

  it('renders context usage bar when provided', () => {
    const { getByTestId } = render(
      <StatusLine
        status={{
          provider: 'Test',
          model: 'm1',
          permissionMode: 'ask',
          contextUsage: { ratio: 0.65 },
        }}
      />,
    );
    expect(getByTestId('status-context')).toBeInTheDocument();
  });

  it('renders session ID when provided', () => {
    const { getByText } = render(
      <StatusLine
        status={{
          provider: 'P',
          model: 'M',
          permissionMode: 'auto',
          sessionId: 'sess-123',
        }}
      />,
    );
    expect(getByText('sess-123')).toBeInTheDocument();
  });
});
