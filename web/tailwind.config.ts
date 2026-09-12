import type { Config } from 'tailwindcss';

export default {
  content: [
    './index.html',
    './src/**/*.{html,js,svelte,ts}',
  ],
  theme: {
    extend: {},
  },
  plugins: [],
} satisfies Config;
