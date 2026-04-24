import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AboutPanel } from './AboutPanel';

describe('AboutPanel', () => {
  afterEach(cleanup);

  it('renders the section title', () => {
    render(<AboutPanel />);
    expect(screen.getByText('关于')).toBeInTheDocument();
  });

  it('renders version number', () => {
    render(<AboutPanel />);
    expect(screen.getByText('0.1.0')).toBeInTheDocument();
  });

  it('renders build ID', () => {
    render(<AboutPanel />);
    expect(screen.getByText('版本')).toBeInTheDocument();
    expect(screen.getByText('构建 ID')).toBeInTheDocument();
  });

  it('renders license info', () => {
    render(<AboutPanel />);
    expect(screen.getByText('MIT')).toBeInTheDocument();
  });

  it('renders project links', () => {
    render(<AboutPanel />);
    expect(screen.getByTestId('link-github')).toBeInTheDocument();
    expect(screen.getByTestId('link-docs')).toBeInTheDocument();
  });

  it('renders system information', () => {
    render(<AboutPanel />);
    expect(screen.getByText('Tauri (Rust)')).toBeInTheDocument();
    expect(screen.getByText('React + TypeScript')).toBeInTheDocument();
    expect(screen.getByText('Tailwind CSS')).toBeInTheDocument();
  });
});
