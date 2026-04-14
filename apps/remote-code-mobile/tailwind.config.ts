import type { Config } from 'tailwindcss';

export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        // Match the existing remote client color palette
        sand: {
          50: '#faf6ef',
          100: '#f7f2e8',
          200: '#f2ebdf',
          300: '#e5ddcf',
          400: '#e0d6c6',
          500: '#ddd2c1',
          600: '#c4b8a4',
        },
      },
      borderRadius: {
        card: '24px',
        panel: '36px',
      },
    },
  },
  plugins: [],
} satisfies Config;
