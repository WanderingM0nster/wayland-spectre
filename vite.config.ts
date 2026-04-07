import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],

	// Vite options tailored for Tauri:
	// prevent vite from obscuring rust errors
	clearScreen: false,
	server: {
		port: 5173,
		strictPort: true,
		host: host || false,
		hmr: host
			? {
					protocol: 'ws',
					host,
					port: 5183
				}
			: undefined,
		watch: {
			// tell vite to ignore watching `src-tauri`
			ignored: ['**/src-tauri/**']
		}
	},
	// to access the Tauri environment variables set by the CLI with information about the current target
	envPrefix: ['VITE_', 'TAURI_ENV_*']
});
