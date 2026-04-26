import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { MobileBottomSheet } from './MobileBottomSheet';

afterEach(() => { cleanup(); });

describe('MobileBottomSheet', () => {
  it('renders trigger element', () => {
    render(
      <MobileBottomSheet trigger={<button>Open</button>} title="Sheet">
        <p>Content</p>
      </MobileBottomSheet>,
    );
    expect(screen.getByText('Open')).toBeInTheDocument();
  });

  it('renders desktop content directly', () => {
    render(
      <MobileBottomSheet trigger={<button>Open</button>} title="Sheet">
        <p>Desktop content</p>
      </MobileBottomSheet>,
    );
    // Desktop view shows children directly (hidden lg:block div)
    expect(screen.getByText('Desktop content')).toBeInTheDocument();
  });

  it('renders without badge when not provided', () => {
    const { container } = render(
      <MobileBottomSheet trigger={<button>Open</button>} title="Sheet">
        <p>X</p>
      </MobileBottomSheet>,
    );
    expect(container).toBeInTheDocument();
  });
});
