import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AttachmentMessage } from './AttachmentMessage';

describe('AttachmentMessage', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<AttachmentMessage fileName="report.pdf" />);
    expect(screen.getByTestId('attachment-message')).toBeInTheDocument();
  });

  it('displays file name', () => {
    render(<AttachmentMessage fileName="report.pdf" />);
    expect(screen.getByText('report.pdf')).toBeInTheDocument();
  });

  it('displays file type', () => {
    render(<AttachmentMessage fileName="report.pdf" fileType="application/pdf" />);
    expect(screen.getByText('application/pdf')).toBeInTheDocument();
  });

  it('displays formatted file size', () => {
    render(<AttachmentMessage fileName="report.pdf" fileSize={2048} />);
    expect(screen.getByText('2.0 KB')).toBeInTheDocument();
  });

  it('displays image preview when provided', () => {
    render(
      <AttachmentMessage
        fileName="photo.png"
        fileType="image/png"
        preview="data:image/png;base64,abc"
      />,
    );
    expect(screen.getByTestId('attachment-preview')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    const { container } = render(
      <AttachmentMessage fileName="a.txt" className="custom" />,
    );
    expect(container.firstChild).toHaveClass('custom');
  });
});
