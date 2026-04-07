// Svelte 5 Runes store for diagnostic state.
// Using .svelte.ts extension so $state/$derived work outside components.
import { invoke } from '@tauri-apps/api/core';
import type { DiagnosticReport, DiagnosticResult, CaptureTestResult, LayerLabel } from '$lib/types';

// ── State ──────────────────────────────────────────────────────────────────

let _report = $state<DiagnosticReport | null>(null);
let _running = $state(false);
let _fixing = $state<string | null>(null); // check name currently being fixed
let _fixLog = $state<string[]>([]);
let _captureResult = $state<CaptureTestResult | null>(null);
let _captureRunning = $state(false);
let _error = $state<string | null>(null);

// ── Derived ────────────────────────────────────────────────────────────────

const summary = $derived.by(() => {
	if (!_report) return { pass: 0, warn: 0, fail: 0 };
	return _report.summary;
});

const resultsByLayer = $derived.by(() => {
	if (!_report) return {} as Record<LayerLabel, DiagnosticResult[]>;
	return _report.results.reduce(
		(acc, r) => {
			const key = r.layer as LayerLabel;
			if (!acc[key]) acc[key] = [];
			acc[key].push(r);
			return acc;
		},
		{} as Record<LayerLabel, DiagnosticResult[]>
	);
});

const hasFailures = $derived(_report !== null && _report.summary.fail > 0);
const hasWarnings = $derived(_report !== null && _report.summary.warn > 0);

// ── Actions ────────────────────────────────────────────────────────────────

async function runDiagnostics() {
	_running = true;
	_error = null;
	_report = null;
	try {
		_report = await invoke<DiagnosticReport>('run_diagnostics');
	} catch (e) {
		_error = String(e);
	} finally {
		_running = false;
	}
}

async function executeFix(checkName: string, fixCommand: string) {
	_fixing = checkName;
	_fixLog = [];
	try {
		const output = await invoke<string>('execute_fix', { fixCommand });
		_fixLog = output.split('\n').filter(Boolean);
		// Re-run diagnostics after fix
		await runDiagnostics();
	} catch (e) {
		_error = String(e);
	} finally {
		_fixing = null;
	}
}

async function runCaptureTest() {
	_captureRunning = true;
	_captureResult = null;
	try {
		_captureResult = await invoke<CaptureTestResult>('run_capture_test');
	} catch (e) {
		_error = String(e);
	} finally {
		_captureRunning = false;
	}
}

async function generateBugReport(): Promise<string | null> {
	try {
		return await invoke<string>('generate_bug_report');
	} catch (e) {
		_error = String(e);
		return null;
	}
}

function clearError() {
	_error = null;
}

// ── Exported reactive object ───────────────────────────────────────────────

export const diagnostic = {
	// State (reactive via getters)
	get report() { return _report; },
	get running() { return _running; },
	get fixing() { return _fixing; },
	get fixLog() { return _fixLog; },
	get captureResult() { return _captureResult; },
	get captureRunning() { return _captureRunning; },
	get error() { return _error; },

	// Derived
	get summary() { return summary; },
	get resultsByLayer() { return resultsByLayer; },
	get hasFailures() { return hasFailures; },
	get hasWarnings() { return hasWarnings; },

	// Actions
	runDiagnostics,
	executeFix,
	runCaptureTest,
	generateBugReport,
	clearError
};
