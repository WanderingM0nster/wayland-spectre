<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!--
  KwinJournal.svelte  — Session 6
  Collapsible panel displaying recent plasma-kwin_wayland journal entries.

  Features:
    - Collapsed by default; expands on click
    - AUTO-EXPANDS once when autoExpand prop is true (kwin_screencast_effect_active FAIL)
    - Shows amber "auto · effect FAIL" badge in header when auto-expanded
    - Invokes the `get_kwin_journal` Tauri command on first expand and on refresh
    - Quick-filter chips: All · screencast · effect · crtc · error
    - Line-level colour coding (error → red, warn → amber, screencast/effect → accent)
    - Auto-scroll to bottom on load; manual scroll preserved after
    - Copy-to-clipboard button for the full log

  Session 6 changes:
    - Added `autoExpand` prop — when it becomes true the panel opens automatically
      and fires the journal fetch. Gated by `autoExpandedOnce` so the user can
      close the panel freely; it will not fight them by re-opening.
    - Added amber "auto · effect FAIL" badge in the header to communicate why
      the panel opened on its own.
-->
<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';

	// ── Props ────────────────────────────────────────────────────────────────
	let { autoExpand = false }: { autoExpand?: boolean } = $props();

	// ── State ───────────────────────────────────────────────────────────────
	let open = $state(false);
	let lines = $state<string[]>([]);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let activeFilter = $state<string>('');
	let copyFeedback = $state(false);
	let logEl = $state<HTMLElement | null>(null);
	/**
	 * Tracks whether the auto-expand has fired at least once.
	 * Gating on this (not on `open`) prevents the effect from
	 * fighting the user — once expanded, the panel stays under
	 * manual control regardless of the autoExpand prop value.
	 */
	let autoExpandedOnce = $state(false);

	const QUICK_FILTERS = ['screencast', 'effect', 'crtc', 'error', 'format'];

	// ── Auto-expand: fires once when kwin_screencast_effect_active is FAIL ──
	$effect(() => {
		if (autoExpand && !autoExpandedOnce) {
			open = true;
			autoExpandedOnce = true;
			if (lines.length === 0 && !error) {
				fetchJournal();
			}
		}
	});

	// ── Actions ─────────────────────────────────────────────────────────────

	async function fetchJournal(filter = activeFilter) {
		loading = true;
		error = null;
		try {
			lines = await invoke<string[]>('get_kwin_journal', {
				lines: 80,
				filter: filter || null,
			});
		} catch (e) {
			error = String(e);
			lines = [];
		} finally {
			loading = false;
			// Scroll to bottom after render
			setTimeout(() => {
				if (logEl) logEl.scrollTop = logEl.scrollHeight;
			}, 0);
		}
	}

	function toggle() {
		open = !open;
		if (open && lines.length === 0 && !error) {
			fetchJournal();
		}
	}

	function setFilter(f: string) {
		activeFilter = activeFilter === f ? '' : f;
		fetchJournal(activeFilter);
	}

	async function copyLog() {
		await navigator.clipboard.writeText(lines.join('\n'));
		copyFeedback = true;
		setTimeout(() => (copyFeedback = false), 1500);
	}

	// ── Line classifier ─────────────────────────────────────────────────────
	type LineKind = 'error' | 'warn' | 'accent' | 'dim' | 'normal';

	function classifyLine(line: string): LineKind {
		const l = line.toLowerCase();
		if (l.includes('error') || l.includes('fail') || l.includes('crtc') || l.includes('abort'))
			return 'error';
		if (l.includes('warn')) return 'warn';
		if (l.includes('screencast') || l.includes('effect')) return 'accent';
		if (l.startsWith('--')) return 'dim'; // journalctl separator lines
		return 'normal';
	}

	const kindClass: Record<LineKind, string> = {
		error:  'text-status-fail',
		warn:   'text-status-warn',
		accent: 'text-status-pass',
		dim:    'text-muted-foreground opacity-50',
		normal: 'text-foreground/80',
	};
</script>

<!-- ── Panel wrapper ───────────────────────────────────────────────────── -->
<div class="mt-4 rounded-lg border border-border bg-card">

	<!-- Header / toggle row -->
	<button
		class="flex w-full items-center justify-between px-4 py-2.5 text-left hover:bg-muted/30 transition-colors rounded-t-lg"
		onclick={toggle}
		aria-expanded={open}
	>
		<div class="flex items-center gap-2">
			<span class="text-sm font-semibold text-foreground">KWin Journal</span>
			<span class="rounded-full bg-muted px-1.5 py-0.5 text-xs text-muted-foreground font-mono">
				plasma-kwin_wayland
			</span>
			{#if lines.length > 0}
				<span class="rounded-full bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
					{lines.length} lines
				</span>
			{/if}
			<!-- Amber badge — visible once kwin_screencast_effect_active FAIL triggered open -->
			{#if autoExpandedOnce}
				<span
					class="rounded-full border border-status-warn/40 bg-status-warn/10 px-1.5 py-0.5 text-[10px] font-medium text-status-warn"
					title="Opened automatically because kwin_screencast_effect_active is FAIL"
				>
					auto · effect FAIL
				</span>
			{/if}
		</div>
		<span class="text-muted-foreground text-sm select-none">{open ? '▲' : '▼'}</span>
	</button>

	{#if open}
		<!-- Toolbar -->
		<div class="flex flex-wrap items-center gap-1.5 border-t border-border px-3 py-2">
			<!-- Quick-filter chips -->
			{#each QUICK_FILTERS as f}
				<button
					class="rounded-full border px-2.5 py-0.5 text-xs font-mono transition-colors {
						activeFilter === f
							? 'border-primary bg-primary/10 text-primary'
							: 'border-border text-muted-foreground hover:border-primary/50 hover:text-foreground'
					}"
					onclick={() => setFilter(f)}
				>{f}</button>
			{/each}

			<span class="ml-auto flex items-center gap-1.5">
				<!-- Refresh -->
				<button
					class="rounded border border-border px-2.5 py-0.5 text-xs text-muted-foreground hover:text-foreground hover:border-primary/50 transition-colors disabled:opacity-40"
					disabled={loading}
					onclick={() => fetchJournal()}
				>
					{loading ? '…' : '↻ Refresh'}
				</button>
				<!-- Copy -->
				<button
					class="rounded border border-border px-2.5 py-0.5 text-xs text-muted-foreground hover:text-foreground hover:border-primary/50 transition-colors disabled:opacity-40"
					disabled={lines.length === 0}
					onclick={copyLog}
				>
					{copyFeedback ? '✓ Copied' : '⎘ Copy'}
				</button>
			</span>
		</div>

		<!-- Log output -->
		<div
			bind:this={logEl}
			class="h-64 overflow-y-auto border-t border-border bg-background/50 px-3 py-2"
		>
			{#if loading}
				<p class="text-xs text-muted-foreground animate-pulse py-2">Loading journal…</p>
			{:else if error}
				<p class="text-xs text-status-fail py-2">Error: {error}</p>
			{:else if lines.length === 0}
				<p class="text-xs text-muted-foreground py-2">No entries.</p>
			{:else}
				{#each lines as line}
					<p class="font-mono text-[11px] leading-5 whitespace-pre-wrap break-all {kindClass[classifyLine(line)]}">
						{line}
					</p>
				{/each}
			{/if}
		</div>

		<!-- Hint -->
		<p class="border-t border-border px-3 py-1.5 text-[11px] text-muted-foreground">
			Tip: filter by <span class="font-mono">effect</span> or <span class="font-mono">crtc</span>
			to see the screencast plugin startup failure.
			<a
				href="https://bugs.kde.org/show_bug.cgi?id=493277"
				target="_blank"
				rel="noopener noreferrer"
				class="text-primary underline-offset-2 hover:underline ml-1"
			>KDE bug 493277</a>
		</p>
	{/if}
</div>
