import { ref, watch } from "vue";

const THEMES = {
	dark: "dark",
	light: "light",
	purple: "purple",
};

// --- LOCAL PATCH: namespaced theme localStorage key (1/2) ---
// Reason: under Home Assistant Ingress the UI shares localStorage with the
// HA frontend, which uses the same unnamespaced "selectedTheme" key for its
// own theme settings (stored as a JSON object, rewritten on every HA boot).
// Reading it verbatim broke the theme, and writing it clobbered HA's value.
// "deemix-selectedTheme" is touched only by this app (same key in index.html).
// Original (upstream deemix-webui@4.7.0):
// const initialTheme =
// 	localStorage.getItem("selectedTheme") ||
// 	document.documentElement.dataset.theme ||
// 	THEMES.dark;
const SELECTED_THEME_KEY = "deemix-selectedTheme";

const initialTheme =
	localStorage.getItem(SELECTED_THEME_KEY) ||
	document.documentElement.dataset.theme ||
	THEMES.dark;
// --- END LOCAL PATCH ---
const currentTheme = ref(initialTheme);

watch(currentTheme, (newTheme, oldTheme) => {
	// No operation needed
	if (oldTheme === newTheme) return;

	// --- LOCAL PATCH: namespaced theme localStorage key (2/2) ---
	// Original (upstream deemix-webui@4.7.0):
	// 	localStorage.setItem("selectedTheme", newTheme);
	localStorage.setItem(SELECTED_THEME_KEY, newTheme);
	// --- END LOCAL PATCH ---
	document.documentElement.dataset.theme = newTheme;

	animateAllElements();
});

function animateAllElements() {
	// Animating everything to have a smoother theme switch
	const allElements = document.querySelectorAll("*");

	allElements.forEach((el) => {
		el.classList.add("changing-theme");
	});

	document.documentElement.addEventListener(
		"transitionend",
		function transitionHandler() {
			allElements.forEach((el) => {
				el.classList.remove("changing-theme");
			});

			document.documentElement.removeEventListener(
				"transitionend",
				transitionHandler
			);
		}
	);
}

export const useTheme = () => ({
	THEMES,
	currentTheme,
});
