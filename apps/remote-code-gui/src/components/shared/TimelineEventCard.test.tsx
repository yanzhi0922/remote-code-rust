import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { TimelineEventCard } from './TimelineEventCard';

afterEach(() => { cleanup(); });

describe('TimelineEventCard', () => {
  it('renders eyebrow text', () => {
    render(
      <TimelineEventCard eyebrow="Tool" accent="text-emerald-700" icon={<span>🔧</span>} timestampLabel="12:00">
        <p>Content</p>
      </TimelineEventCard>,
    );
    expect(screen.getByText('Tool')).toBeInTheDocument();
  });

  it('renders children content', () => {
    render(
      <TimelineEventCard eyebrow="Step" accent="text-blue-700" icon={null} timestampLabel="">
        <p>Details here</p>
      </TimelineEventCard>,
    );
    expect(screen.getByText('Details here')).toBeInTheDocument();
  });

  it('renders timestamp', () => {
    render(
      <TimelineEventCard eyebrow="E" accent="" icon={null} timestampLabel="3:45 PM">
        <p>X</p>
      </TimelineEventCard>,
    );
    expect(screen.getByText('3:45 PM')).toBeInTheDocument();
  });
});
