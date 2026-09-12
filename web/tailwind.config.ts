import type { Config } from 'tailwindcss';

export default {
  content: [
    './index.html',
    './src/**/*.{html,js,svelte,ts}',
  ],
  darkMode: 'class',
  theme: {
    extend: {},
  },
  plugins: [],
} satisfies Config;
