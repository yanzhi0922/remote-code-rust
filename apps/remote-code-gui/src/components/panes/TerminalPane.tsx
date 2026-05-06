import { useEffect, useRef, useCallback } from 'react';
import { Terminal as TerminalIcon } from 'lucide-react';
import type { Terminal } from '@xterm/xterm';

export interface TerminalHandle {
  write: (data: string) => void;
  clear: () => void;
  getTerminal: () => Terminal | null;
}

declare global {
  interface HTMLElement {
    __terminal?: TerminalHandle;
  }
}

interface TerminalPaneProps {
  className?: string;
  onInput?: (data: string) => void;
  onResize?: (cols: number, rows: number) => void;
}

export function TerminalPane({ className = '', onInput, onResize }: TerminalPaneProps) {
  const termRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<Terminal | null>(null);
  const fitRef = useRef<import('@xterm/addon-fit').FitAddon | null>(null);
  const initialized = useRef(false);

  useEffect(() => {
    if (!termRef.current || initialized.current) return;
    initialized.current = true;

    let disposed = false;

    Promise.all([
      import('@xterm/xterm'),
      import('@xterm/addon-fit'),
    ]).then(([{ Terminal }, { FitAddon }]) => {
      if (disposed || !termRef.current) return;

      const term = new Terminal({
        fontSize: 13,
        fontFamily: "'JetBrains Mono', 'Cascadia Code', 'Fira Code', Menlo, Monaco, 'Courier New', monospace",
        theme: {
          background: '#0d1117',
          foreground: '#c9d1d9',
          cursor: '#58a6ff',
          cursorAccent: '#0d1117',
          selectionBackground: '#264f78',
          black: '#484f58',
          red: '#ff7b72',
          green: '#3fb950',
          yellow: '#d29922',
          blue: '#58a6ff',
          magenta: '#bc8cff',
          cyan: '#39c5cf',
          white: '#b1bac4',
          brightBlack: '#6e7681',
          brightRed: '#ffa198',
          brightGreen: '#56d364',
          brightYellow: '#e3b341',
          brightBlue: '#79c0ff',
          brightMagenta: '#d2a8ff',
          brightCyan: '#56d4dd',
          brightWhite: '#f0f6fc',
        },
        cursorBlink: true,
        scrollback: 5000,
        allowProposedApi: true,
      });

      const fitAddon = new FitAddon();
      term.loadAddon(fitAddon);
      term.open(termRef.current);
      fitAddon.fit();

      xtermRef.current = term;
      fitRef.current = fitAddon;

      term.onData((data: string) => {
        onInput?.(data);
      });

      const observer = new ResizeObserver(() => {
        if (fitRef.current && xtermRef.current) {
          try {
            fitRef.current.fit();
            const { cols, rows } = xtermRef.current;
            onResize?.(cols, rows);
          } catch {
            // fit() may throw during resize transitions
          }
        }
      });
      observer.observe(termRef.current);

      term.writeln('\x1b[1;36m$ Remote Code Terminal\x1b[0m');
      term.writeln('\x1b[90mReady. Commands from Agent will appear here.\x1b[0m');
      term.writeln('');

      return () => observer.disconnect();
    });

    return () => {
      disposed = true;
      xtermRef.current?.dispose();
      xtermRef.current = null;
      fitRef.current = null;
    };
  }, []);

  const writeData = useCallback((data: string) => {
    xtermRef.current?.write(data);
  }, []);

  const clearTerm = useCallback(() => {
    xtermRef.current?.clear();
  }, []);

  // Expose imperative API via DOM data attribute for parent access
  useEffect(() => {
    if (termRef.current) {
      termRef.current.__terminal = {
        write: writeData,
        clear: clearTerm,
        getTerminal: () => xtermRef.current,
      };
    }
  }, [writeData, clearTerm]);

  return (
    <div className={`flex h-full flex-col bg-rc-bg-code ${className}`}>
      <div className="flex items-center gap-2 border-b border-rc-border-primary px-3 py-1.5">
        <TerminalIcon size={14} className="text-rc-text-inverse" />
        <span className="text-xs font-medium text-rc-text-inverse">Terminal</span>
        <button
          type="button"
          onClick={clearTerm}
          className="ml-auto text-2xs text-rc-text-tertiary hover:text-rc-text-inverse transition-colors"
        >
          清屏
        </button>
      </div>
      <div ref={termRef} className="flex-1 min-h-0" />
    </div>
  );
}