import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import { WizardNavigationFooter } from './WizardNavigationFooter';

describe('WizardNavigationFooter', () => {
  afterEach(() => { cleanup(); });

  it('renders footer with default instructions', () => {
    const { getByTestId, getByText } = render(<WizardNavigationFooter />);
    expect(getByTestId('wizard-navigation-footer')).toBeInTheDocument();
    expect(getByText(/navigate/)).toBeInTheDocument();
    expect(getByText(/select/)).toBeInTheDocument();
    expect(getByText(/go back/)).toBeInTheDocument();
  });

  it('renders custom instructions when provided', () => {
    const { getByText } = render(
      <WizardNavigationFooter instructions={<span>Custom footer text</span>} />,
    );
    expect(getByText('Custom footer text')).toBeInTheDocument();
  });
});
