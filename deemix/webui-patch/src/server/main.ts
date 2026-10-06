import { DeemixApp } from "@/deemixApp.js";
import { logger, removeOldLogs } from "@/helpers/logger.js";
import { loadLoginCredentials } from "@/helpers/loginStorage.js";
import cookieParser from "cookie-parser";
import { utils, type Listener } from "deemix";
import express, { type Express } from "express";
import session from "express-session";
import memorystore from "memorystore";
import morgan from "morgan";
// --- LOCAL PATCH: readFile import for the X-Ingress-Path base injection (see the production block below) ---
// Original: no node:fs import.
import { readFile } from "node:fs";
// --- END LOCAL PATCH ---
import { dirname, join } from "path";
import { fileURLToPath } from "url";
import ViteExpress from "vite-express";
import { WebSocket, WebSocketServer } from "ws";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import { normalizePort } from "./helpers/port.js";
import { getErrorCb, getListeningCb } from "./helpers/server-callbacks.js";
import { registerApis } from "./routes/api/register.js";
import indexRouter from "./routes/index.js";
import type { Arguments } from "./types.js";
import { registerWebsocket } from "./websocket/index.js";

const MemoryStore = memorystore(session);

// TODO: Remove type assertion while keeping correct types
const argv = yargs(hideBin(process.argv)).options({
	port: { type: "string", default: "6595" },
	host: { type: "string", default: "0.0.0.0" },
	locationbase: { type: "string", default: "/" },
	singleuser: { type: "boolean", default: false },
}).argv as Arguments;

const serverPort = process.env.DEEMIX_SERVER_PORT ?? argv.port;
const deemixHost = process.env.DEEMIX_HOST ?? argv.host;
const isSingleUser =
	process.env.DEEMIX_SINGLE_USER === undefined
		? !!argv.singleuser
		: process.env.DEEMIX_SINGLE_USER === "true";

const app: Express = express();

if (isSingleUser) loadLoginCredentials();

app.set("isSingleUser", isSingleUser);

/* === Deemix App === */
const listener: Listener = {
	send: (key: string, data?: any) => {
		const logLine = utils.formatListener(key, data);
		if (logLine) logger.info(logLine);
		if (["downloadInfo", "downloadWarn"].includes(key)) return;
		wss.clients.forEach((client) => {
			if (client.readyState === WebSocket.OPEN) {
				client.send(JSON.stringify({ key, data }));
			}
		});
	},
};
const deemixApp = new DeemixApp(listener);

/* === Middlewares === */
app.use(express.json());
app.use(express.urlencoded({ extended: false }));
app.use(cookieParser());
app.use(
	// @ts-expect-error
	session({
		store: new MemoryStore({
			checkPeriod: 86400000, // prune expired entries every 24h
		}),
		secret: "U2hoLCBpdHMgYSBzZWNyZXQh",
		resave: true,
		saveUninitialized: true,
	})
);

if (process.env.NODE_ENV === "development") {
	app.use(morgan("dev"));
}

/* === Routes === */
app.use("/", indexRouter);

/* === APIs === */
registerApis(app);

/* === Config === */
app.set("port", serverPort);
app.set("deemix", deemixApp);

/* === Server port === */
const server = app.listen({
	port: normalizePort(serverPort),
	host: deemixHost,
});
const wss = new WebSocketServer({ server });

if (process.env.NODE_ENV === "production") {
	const publicPath = join(dirname(fileURLToPath(import.meta.url)), "public");
	// --- LOCAL PATCH: inject base path from X-Ingress-Path into index.html ---
	// Reason: under Home Assistant Ingress the UI is served from
	// /api/hassio_ingress/<token>/, so document-relative asset URLs and the
	// location.base global (API calls, router, websocket) must carry that
	// prefix. Injecting <base href> + window.location.base into index.html
	// fixes asset resolution at ANY route depth (the SPA fallback otherwise
	// serves index.html for asset requests on deep-link refresh). Without the
	// header (direct access) the base is "/". Original (upstream
	// deemix-webui@4.7.0):
	// 	app.use(express.static(publicPath));
	// 	app.get("*", (_, res) => {
	// 		res.sendFile(join(publicPath, "index.html"));
	// 	});
	app.use(express.static(publicPath, { index: false }));
	app.get("*", (req, res) => {
		const ingressPath = req.headers["x-ingress-path"];
		const base =
			typeof ingressPath === "string" &&
			/^\/[\w.\-~!$&'()*+,;=:@%]*\/?$/.test(ingressPath)
				? ingressPath.replace(/\/+$/, "") + "/"
				: "/";
		readFile(join(publicPath, "index.html"), "utf8", (err, html) => {
			if (err) return res.status(500).send("Cannot load index.html");
			res.type("html").send(
				html.replace(
					"<head>",
					`<head><base href="${base}"><script>window.location.base="${base}";</script>`
				)
			);
		});
	});
	// --- END LOCAL PATCH ---
} else {
	ViteExpress.bind(app, server);
}

/* === Server callbacks === */
server.on("error", getErrorCb(serverPort));
server.on("listening", getListeningCb(server));
registerWebsocket(wss, deemixApp);

/* === Remove Old logs files === */
removeOldLogs(5);

export { app, deemixApp, server };
