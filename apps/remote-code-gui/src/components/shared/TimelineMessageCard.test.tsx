import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { TimelineMessageCard } from './TimelineMessageCard';

afterEach(() => { cleanup(); });

describe('TimelineMessageCard', () => {
  it('renders user role message', () => {
    render(
      <TimelineMessageCard role="user" header="You">
        <p>User message</p>
      </TimelineMessageCard>,
    );
    expect(screen.getByText('You')).toBeInTheDocument();
    expect(screen.getByText('User message')).toBeInTheDocument();
  });

  it('renders assistant role message', () => {
    render(
      <TimelineMessageCard role="assistant" header="Assistant">
        <p>AI response</p>
      </TimelineMessageCard>,
    );
    expect(screen.getByText('Assistant')).toBeInTheDocument();
    expect(screen.getByText('AI response')).toBeInTheDocument();
  });
});
