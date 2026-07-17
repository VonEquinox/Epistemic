/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        // Material Design 3 semantic tokens (values in index.css :root)
        primary: {
          DEFAULT: 'var(--md-primary)',
          container: 'var(--md-primary-container)',
        },
        'on-primary': {
          DEFAULT: 'var(--md-on-primary)',
          container: 'var(--md-on-primary-container)',
        },
        secondary: {
          DEFAULT: 'var(--md-secondary)',
          container: 'var(--md-secondary-container)',
        },
        'on-secondary': {
          DEFAULT: 'var(--md-on-secondary)',
          container: 'var(--md-on-secondary-container)',
        },
        tertiary: {
          DEFAULT: 'var(--md-tertiary)',
          container: 'var(--md-tertiary-container)',
        },
        'on-tertiary': {
          DEFAULT: 'var(--md-on-tertiary)',
          container: 'var(--md-on-tertiary-container)',
        },
        error: {
          DEFAULT: 'var(--md-error)',
          container: 'var(--md-error-container)',
        },
        'on-error': {
          DEFAULT: 'var(--md-on-error)',
          container: 'var(--md-on-error-container)',
        },
        surface: {
          DEFAULT: 'var(--md-surface)',
          dim: 'var(--md-surface-dim)',
          container: 'var(--md-surface-container)',
          'container-lowest': 'var(--md-surface-container-lowest)',
          'container-low': 'var(--md-surface-container-low)',
          'container-high': 'var(--md-surface-container-high)',
          'container-highest': 'var(--md-surface-container-highest)',
        },
        'on-surface': {
          DEFAULT: 'var(--md-on-surface)',
          variant: 'var(--md-on-surface-variant)',
        },
        outline: {
          DEFAULT: 'var(--md-outline)',
          variant: 'var(--md-outline-variant)',
        },
        'inverse-surface': 'var(--md-inverse-surface)',
        'inverse-on-surface': 'var(--md-inverse-on-surface)',
        'inverse-primary': 'var(--md-inverse-primary)',

        // Legacy scale, remapped onto the MD3 neutral tones so existing
        // ink-* classes across pages pick up the new theme automatically.
        ink: {
          50: '#F9F9FF',
          100: '#F3F3FA',
          200: '#E2E2E9',
          300: '#C4C6D0',
          400: '#8F9199',
          500: '#74777F',
          600: '#5C5F67',
          700: '#44474E',
          800: '#2E3036',
          900: '#191C20',
          950: '#0D0E11',
        },
        accent: {
          DEFAULT: '#0B57D0',
          soft: '#D3E3FD',
        },
        dispute: '#B3261E',
        candidate: '#74777F',
      },
      fontFamily: {
        sans: ['Roboto', '"Noto Sans SC"', 'system-ui', 'sans-serif'],
        mono: ['"Roboto Mono"', 'ui-monospace', 'monospace'],
      },
      boxShadow: {
        // MD3 elevation levels
        elev1: '0 1px 2px rgba(0,0,0,0.3), 0 1px 3px 1px rgba(0,0,0,0.15)',
        elev2: '0 1px 2px rgba(0,0,0,0.3), 0 2px 6px 2px rgba(0,0,0,0.15)',
        elev3: '0 1px 3px rgba(0,0,0,0.3), 0 4px 8px 3px rgba(0,0,0,0.15)',
      },
    },
  },
  plugins: [],
};
