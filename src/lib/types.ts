// Mirror of src-tauri/src/domain/types.rs
// These must stay in sync with the Rust structs — JSON is the contract.

export type CheckStatus = 'PASS' | 'WARN' | 'FAIL' | 'SKIP';
export type Confidence = 'HIGH' | 'MEDIUM' | 'LOW';
export type SessionType = 'WAYLAND' | 'X11' | 'UNKNOWN';

export type LayerLabel =
	| 'L0' // NVIDIA / GPU driver
	| 'L1' // D-Bus / portal session
	| 'L2' // Compositor connection / identity
	| 'L3' // Wayland compositor protocols
	| 'L4' // PipeWire graph
	| 'L5' // Flatpak permissions
	| 'L6' // Environment variables
	| 'L7'; // KWin plugins

export interface DiagnosticResult {
	layer: LayerLabel;
	check: string;
	status: CheckStatus;
	detail: string;
	fix: string | null;
	confidence: Confidence;
}

export interface DiagnosticSummary {
	pass: number;
	warn: number;
	fail: number;
}

export interface SystemInfo {
	generated_at: string;
	hostname: string;
	kernel: string;
	nvidia_driver: string | null;
	bazzite_image: string | null;
	session_type: SessionType;
}

export interface DiagnosticReport {
	schema_version: string;
	system: SystemInfo;
	results: DiagnosticResult[];
	summary: DiagnosticSummary;
}

export interface CaptureTestResult {
	success: boolean;
	node_id: number | null;
	width: number | null;
	height: number | null;
	format: string | null;
	error: string | null;
}

// Layer metadata for display
export const LAYER_META: Record<
	LayerLabel,
	{ name: string; description: string; icon: string }
> = {
	L0: { name: 'GPU / NVIDIA', description: 'Driver version, DMA-BUF modifiers, EGL Wayland', icon: 'memory' },
	L1: { name: 'D-Bus / Portal Session', description: 'Zombie sessions, portal service health', icon: 'settings_ethernet' },
	L2: { name: 'Compositor Connection', description: 'Wayland socket, compositor identity', icon: 'hub' },
	L3: { name: 'Wayland Protocols', description: 'zkde_screencast, ext_image, syncobj globals', icon: 'display_settings' },
	L4: { name: 'PipeWire', description: 'Node state, screencast graph', icon: 'cable' },
	L5: { name: 'Flatpak Permissions', description: 'Stale deny entries in permission store', icon: 'lock' },
	L6: { name: 'Environment', description: 'WAYLAND_DISPLAY, XDG_CURRENT_DESKTOP in systemd', icon: 'terminal' },
	L7: { name: 'KWin Plugins', description: 'screencast plugin enabled in kwinrc', icon: 'extension' }
};

export const STATUS_COLOURS: Record<CheckStatus, string> = {
	PASS: 'text-status-pass',
	WARN: 'text-status-warn',
	FAIL: 'text-status-fail',
	SKIP: 'text-status-skip'
};

export const STATUS_BG: Record<CheckStatus, string> = {
	PASS: 'bg-status-pass/10 border-status-pass/30',
	WARN: 'bg-status-warn/10 border-status-warn/30',
	FAIL: 'bg-status-fail/10 border-status-fail/30',
	SKIP: 'bg-muted/30 border-border'
};
