/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        ink: {
          50: '#f7f7f5',
          100: '#ecece8',
          200: '#d6d6cf',
          300: '#b3b3a8',
          400: '#8a8a7c',
          500: '#6b6b5e',
          600: '#545449',
          700: '#45453c',
          800: '#3a3a33',
          900: '#32322c',
          950: '#1a1a16',
        },
        accent: {
          DEFAULT: '#2563eb',
          soft: '#dbeafe',
        },
        dispute: '#dc2626',
        candidate: '#9ca3af',
      },
      fontFamily: {
        sans: ['"IBM Plex Sans"', 'system-ui', 'sans-serif'],
        mono: ['"IBM Plex Mono"', 'ui-monospace', 'monospace'],
      },
    },
  },
  plugins: [],
};
