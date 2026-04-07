<script lang="ts">
	import type { CaptureTestResult } from '$lib/types';
	import { cn } from '$lib/utils';

	interface Props {
		result: CaptureTestResult | null;
		running: boolean;
		onRun: () => void;
	}

	let { result, running, onRun }: Props = $props();
</script>

<div class="rounded-lg border border-border bg-muted/10 p-4">
	<div class="mb-3 flex items-center justify-between">
		<div>
			<h3 class="font-semibold">Live Capture Test</h3>
			<p class="text-xs text-muted-foreground">
				Requests a PipeWire screencast node via xdg-desktop-portal
			</p>
		</div>
		<button
			class={cn(
				'rounded-md border border-border px-3 py-1.5 text-sm font-medium',
				'hover:border-primary/50 hover:text-primary transition-colors duration-150',
				running && 'cursor-wait opacity-60'
			)}
			disabled={running}
			onclick={onRun}
		>
			{#if running}
				<span class="inline-flex items-center gap-1">
					<span class="animate-spin">⟳</span>
					Testing…
				</span>
			{:else}
				▶ Run test
			{/if}
		</button>
	</div>

	{#if result}
		<div
			class={cn(
				'rounded-md border p-3',
				result.success
					? 'border-status-pass/30 bg-status-pass/10'
					: 'border-status-fail/30 bg-status-fail/10'
			)}
		>
			{#if result.success}
				<p class="font-medium text-status-pass">✓ Capture succeeded</p>
				<div class="mt-1 grid grid-cols-3 gap-2 font-mono text-xs text-muted-foreground">
					{#if result.node_id !== null}
						<span>Node: {result.node_id}</span>
					{/if}
					{#if result.width && result.height}
						<span>Size: {result.width}×{result.height}</span>
					{/if}
					{#if result.format}
						<span>Format: {result.format}</span>
					{/if}
				</div>
			{:else}
				<p class="font-medium text-status-fail">✗ Capture failed</p>
				{#if result.error}
					<p class="mt-1 font-mono text-xs text-muted-foreground">{result.error}</p>
				{/if}
			{/if}
		</div>
	{:else}
		<p class="text-center text-xs text-muted-foreground">
			Run the test to verify end-to-end screen sharing works
		</p>
	{/if}
</div>
