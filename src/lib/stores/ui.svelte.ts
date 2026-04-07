// SPDX-License-Identifier: GPL-3.0-or-later
// UI preferences store — zoom level with localStorage persistence.
// SSR is disabled for this app (ssr=false in +layout.ts) so localStorage
// and document access are safe at module level.
//
// Session 4 change: first-run default is now 150% (optimal for 32" 4K).
// Users who already have a stored preference are unaffected.

const ZOOM_KEY = 'wayland-spectre-zoom';

// Discrete zoom steps — enough range for 1080p through 4K native (scale=1)
const ZOOM_STEPS = [0.75, 0.85, 1.0, 1.15, 1.3, 1.5, 1.75, 2.0] as const;

// 150% is the recommended default for a 32" 4K display at normal viewing
// distance (≈ matches a 24" 1080p at 100%). Applied on first run only;
// existing stored preferences are respected.
const DEFAULT_ZOOM = 1.5;
const BASE_FONT_PX = 16;

// ── State ──────────────────────────────────────────────────────────────────

const hasStored =
	typeof localStorage !== 'undefined' && localStorage.getItem(ZOOM_KEY) !== null;

const stored = hasStored
	? parseFloat(localStorage.getItem(ZOOM_KEY)!)
	: DEFAULT_ZOOM;

// On first run (no stored value), persist the default immediately so that
// subsequent loads see it as a user preference and not a first-run event.
if (!hasStored && typeof localStorage !== 'undefined') {
	localStorage.setItem(ZOOM_KEY, String(DEFAULT_ZOOM));
}

// Clamp to nearest valid step on load (handles stale/corrupt values)
let _zoom = $state<number>(
	ZOOM_STEPS.reduce((prev, cur) =>
		Math.abs(cur - stored) < Math.abs(prev - stored) ? cur : prev
	)
);

// ── Apply ──────────────────────────────────────────────────────────────────

function applyZoom(z: number) {
	if (typeof document !== 'undefined') {
		document.documentElement.style.fontSize = `${BASE_FONT_PX * z}px`;
	}
}

// Apply immediately on module load
applyZoom(_zoom);

// ── Actions ────────────────────────────────────────────────────────────────

function zoomIn() {
	const next = ZOOM_STEPS.find((s) => s > _zoom) ?? ZOOM_STEPS[ZOOM_STEPS.length - 1];
	_zoom = next;
	applyZoom(_zoom);
	localStorage.setItem(ZOOM_KEY, String(_zoom));
}

function zoomOut() {
	const prev = [...ZOOM_STEPS].reverse().find((s) => s < _zoom) ?? ZOOM_STEPS[0];
	_zoom = prev;
	applyZoom(_zoom);
	localStorage.setItem(ZOOM_KEY, String(_zoom));
}

function resetZoom() {
	_zoom = DEFAULT_ZOOM;
	applyZoom(_zoom);
	localStorage.setItem(ZOOM_KEY, String(_zoom));
}

// ── Export ─────────────────────────────────────────────────────────────────

export const ui = {
	get zoom() { return _zoom; },
	get zoomPct() { return Math.round(_zoom * 100); },
	get isDefault() { return _zoom === DEFAULT_ZOOM; },
	get canZoomIn() { return _zoom < ZOOM_STEPS[ZOOM_STEPS.length - 1]; },
	get canZoomOut() { return _zoom > ZOOM_STEPS[0]; },
	zoomIn,
	zoomOut,
	resetZoom,
};
