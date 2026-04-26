import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import CollapsibleBlock from './CollapsibleBlock';

afterEach(() => { cleanup(); });

describe('CollapsibleBlock', () => {
  it('renders the summary text', () => {
    render(
      <CollapsibleBlock summary="Details">
        <p>Hidden content</p>
      </CollapsibleBlock>,
    );
    expect(screen.getByText('Details')).toBeInTheDocument();
  });

  it('hides children by default', () => {
    render(
      <CollapsibleBlock summary="Details">
        <p>Hidden content</p>
      </CollapsibleBlock>,
    );
    expect(screen.queryByText('Hidden content')).not.toBeInTheDocument();
  });

  it('shows children when defaultOpen is true', () => {
    render(
      <CollapsibleBlock summary="Details" defaultOpen>
        <p>Visible content</p>
      </CollapsibleBlock>,
    );
    expect(screen.getByText('Visible content')).toBeInTheDocument();
  });

  it('toggles open/closed on click', () => {
    render(
      <CollapsibleBlock summary="Toggle">
        <p>Toggle content</p>
      </CollapsibleBlock>,
    );
    expect(screen.queryByText('Toggle content')).not.toBeInTheDocument();
    fireEvent.click(screen.getByText('Toggle'));
    expect(screen.getByText('Toggle content')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Toggle'));
    expect(screen.queryByText('Toggle content')).not.toBeInTheDocument();
  });

  it('applies custom className', () => {
    const { container } = render(
      <CollapsibleBlock summary="S" className="custom-class">
        <p>C</p>
      </CollapsibleBlock>,
    );
    expect(container.firstChild).toHaveClass('custom-class');
  });
});
