import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Modal } from './Modal';

afterEach(() => {
  cleanup();
});

describe('Modal', () => {
  it('renders children when visible', () => {
    render(
      <Modal visible onClose={vi.fn()}>
        <p>Modal content</p>
      </Modal>,
    );
    expect(screen.getByTestId('modal-dialog')).toBeInTheDocument();
    expect(screen.getByText('Modal content')).toBeInTheDocument();
  });

  it('does not render when not visible', () => {
    render(
      <Modal visible={false} onClose={vi.fn()}>
        <p>Hidden</p>
      </Modal>,
    );
    expect(screen.queryByTestId('modal-dialog')).not.toBeInTheDocument();
  });

  it('renders title when provided', () => {
    render(
      <Modal visible onClose={vi.fn()} title="My Modal">
        <p>Content</p>
      </Modal>,
    );
    expect(screen.getByText('My Modal')).toBeInTheDocument();
    expect(screen.getByTestId('modal-header')).toBeInTheDocument();
  });

  it('calls onClose when close button is clicked', () => {
    const onClose = vi.fn();
    render(
      <Modal visible onClose={onClose} title="Close me">
        <p>Content</p>
      </Modal>,
    );
    fireEvent.click(screen.getByTestId('modal-close'));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls onClose when backdrop is clicked', () => {
    const onClose = vi.fn();
    render(
      <Modal visible onClose={onClose}>
        <p>Content</p>
      </Modal>,
    );
    fireEvent.click(screen.getByTestId('modal-backdrop'));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls onClose when Escape is pressed', () => {
    const onClose = vi.fn();
    render(
      <Modal visible onClose={onClose}>
        <p>Content</p>
      </Modal>,
    );
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('applies sm size', () => {
    render(
      <Modal visible onClose={vi.fn()} size="sm">
        <p>Small</p>
      </Modal>,
    );
    expect(screen.getByTestId('modal-dialog').className).toContain('max-w-sm');
  });

  it('applies lg size', () => {
    render(
      <Modal visible onClose={vi.fn()} size="lg">
        <p>Large</p>
      </Modal>,
    );
    expect(screen.getByTestId('modal-dialog').className).toContain('max-w-lg');
  });

  it('applies xl size', () => {
    render(
      <Modal visible onClose={vi.fn()} size="xl">
        <p>Extra Large</p>
      </Modal>,
    );
    expect(screen.getByTestId('modal-dialog').className).toContain('max-w-xl');
  });

  it('renders close button even without title', () => {
    render(
      <Modal visible onClose={vi.fn()}>
        <p>No title</p>
      </Modal>,
    );
    expect(screen.getByTestId('modal-close')).toBeInTheDocument();
  });

  it('merges custom className', () => {
    render(
      <Modal visible onClose={vi.fn()} className="my-modal">
        <p>Custom</p>
      </Modal>,
    );
    expect(screen.getByTestId('modal-dialog').className).toContain('my-modal');
  });
});
