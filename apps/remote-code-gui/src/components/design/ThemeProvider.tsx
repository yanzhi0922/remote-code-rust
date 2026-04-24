import { createContext, useContext } from 'react';
import { cn } from '../../lib/utils';

export interface ThemeProviderProps {
  theme?: 'light' | 'dark';
  children: React.ReactNode;
}

interface ThemeContextValue {
  theme: string;
  isDark: boolean;
}

const ThemeContext = createContext<ThemeContextValue>({
  theme: 'light',
  isDark: false,
});

export function useTheme(): ThemeContextValue {
  return useContext(ThemeContext);
}

export function ThemeProvider({ theme = 'light', children }: ThemeProviderProps) {
  const isDark = theme === 'dark';
  const value: ThemeContextValue = { theme, isDark };

  return (
    <ThemeContext.Provider value={value}>
      <div
        data-testid="theme-provider"
        className={cn(
          isDark ? 'bg-slate-900 text-white' : 'bg-white text-slate-800'
        )}
      >
        {children}
      </div>
    </ThemeContext.Provider>
  );
}
