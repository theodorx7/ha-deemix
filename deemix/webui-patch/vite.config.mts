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
			// --- LOCAL PATCH: bundle public font assets instead of root-absolute URLs ---
			// Original: no "/fonts/" alias.
			// Reason: vendor CSS references fonts as url("/fonts/..."); URLs pointing
			// into publicDir are kept root-absolute by Vite and would 404 under
			// Ingress. The alias resolves them to real files, so Vite emits them
			// as bundled assets with relative URLs (see base: "./").
			"/fonts/": fileURLToPath(new URL("public/fonts/", import.meta.url)),
			// --- END LOCAL PATCH ---
		},
	},
	plugins: [vue(), svgLoader()],
	server: { hmr: { port: 3001 } },
});
