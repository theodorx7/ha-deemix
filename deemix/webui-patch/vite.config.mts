import vue from "@vitejs/plugin-vue";
import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import svgLoader from "vite-svg-loader";

// https://vitejs.dev/config/
export default defineConfig({
	// --- LOCAL PATCH: relative base for Home Assistant Ingress ---
	// Original: no `base` option (defaults to "/", root-absolute asset URLs).
	// Reason: under Ingress the UI is served from /api/hassio_ingress/<token>/
	// and root-absolute asset URLs break. Relative base makes asset URLs
	// resolve against the document URL, which works for both direct access
	// (http://host:6595/) and Ingress.
	base: "./",
	// --- END LOCAL PATCH ---
	build: {
		outDir: "dist/public",
	},
	resolve: {
		alias: {
			"@": fileURLToPath(new URL("src/client", import.meta.url)),
		},
	},
	plugins: [vue(), svgLoader()],
	server: { hmr: { port: 3001 } },
});
