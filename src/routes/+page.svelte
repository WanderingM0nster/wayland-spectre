<script lang="ts">
	import { onMount } from 'svelte';
	import { diagnostic } from '$lib/stores/diagnostic.svelte';
	import type { LayerLabel } from '$lib/types';
	import LayerRow from '$lib/components/LayerRow.svelte';
	import SummaryBar from '$lib/components/SummaryBar.svelte';
	import CaptureTest from '$lib/components/CaptureTest.svelte';
	import { cn } from '$lib/utils';

	const LAYER_ORDER: LayerLabel[] = ['L0', 'L1', 'L2', 'L3', 'L4', 'L5', 'L6', 'L7'];

	// Auto-run on mount
	onMount(() => {
		diagnostic.runDiagnostics();
	});
</script>

<div class="flex min-h-screen flex-col bg-background">
	<!-- Header -->
	<header class="border-b border-border px-6 py-4">
		<div class="flex items-center justify-between">
			<div>
				<h1 class="text-lg font-bold tracking-tight">
					<span class="text-primary">wayland</span><span class="text-status-fail">-spectre</span>
				</h1>
				<p class="text-xs text-muted-foreground">Wayland screen sharing diagnostics · KDE Plasma</p>
			</div>

			<button
				class={cn(
					'rounded-md border border-border px-3 py-1.5 text-sm',
					'text-muted-foreground hover:text-primary hover:border-primary/50',
					'transition-colors duration-150',
					!diagnostic.report && 'opacity-50 cursor-not-allowed'
				)}
				disabled={!diagnostic.report}
				onclick={async () => {
					const path = await diagnostic.generateBugReport();
					if (path) alert(`Bug report saved to:\n${path}`);
				}}
			>
				⬇ Bug report
			</button>
		</div>
	</header>

	<!-- Main content -->
	<main class="flex-1 overflow-y-auto px-6 py-4">
		<div class="mx-auto max-w-3xl space-y-4">

			<!-- Error banner -->
			{#if diagnostic.error}
				<div
					class="flex items-start justify-between rounded-lg border border-status-fail/30 bg-status-fail/10 p-3"
				>
					<p class="text-sm text-status-fail">{diagnostic.error}</p>
					<button
						class="ml-2 text-xs text-muted-foreground hover:text-foreground"
						onclick={diagnostic.clearError}
					>✕</button>
				</div>
			{/if}

			<!-- Loading skeleton -->
			{#if diagnostic.running && !diagnostic.report}
				<div class="space-y-3">
					{#each LAYER_ORDER as layer}
						<div class="h-14 animate-pulse rounded-lg bg-muted/30 border border-border"></div>
					{/each}
					<p class="text-center text-xs text-muted-foreground">Running diagnostics…</p>
				</div>

			<!-- Results -->
			{:else if diagnostic.report}
				<SummaryBar
					summary={diagnostic.summary}
					system={diagnostic.report.system}
					running={diagnostic.running}
					onRerun={diagnostic.runDiagnostics}
				/>

				<!-- Fix log (shown while fixing) -->
				{#if diagnostic.fixLog.length > 0}
					<div class="rounded-lg border border-status-warn/30 bg-status-warn/5 p-3">
						<p class="mb-1 text-xs font-medium text-status-warn">Fix output</p>
						<pre class="font-mono text-xs text-muted-foreground">{diagnostic.fixLog.join('\n')}</pre>
					</div>
				{/if}

				<!-- Layer pipeline -->
				<div class="space-y-2">
					{#each LAYER_ORDER as layer}
						{@const results = diagnostic.resultsByLayer[layer] ?? []}
						{#if results.length > 0}
							<LayerRow
								{layer}
								{results}
								fixing={diagnostic.fixing}
								onFix={diagnostic.executeFix}
							/>
						{/if}
					{/each}
				</div>

				<!-- Capture test -->
				<CaptureTest
					result={diagnostic.captureResult}
					running={diagnostic.captureRunning}
					onRun={diagnostic.runCaptureTest}
				/>

			<!-- Empty state -->
			{:else}
				<div class="py-16 text-center">
					<p class="text-muted-foreground">No results yet.</p>
					<button
						class="mt-3 rounded-md border border-border px-4 py-2 text-sm hover:border-primary/50 hover:text-primary transition-colors"
						onclick={diagnostic.runDiagnostics}
					>
						Run diagnostics
					</button>
				</div>
			{/if}
		</div>
	</main>
</div>
