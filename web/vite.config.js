import { defineConfig, loadEnv } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

const env = loadEnv('', process.cwd(), '');
// https://vitejs.dev/config/
export default defineConfig({
  base: env.VITE_BASE_URL,
  plugins: [svelte()],
})
