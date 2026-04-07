<script lang="ts">
	import type { DiagnosticSummary, SystemInfo } from '$lib/types';
	import { cn } from '$lib/utils';

	interface Props {
		summary: DiagnosticSummary;
		system: SystemInfo;
		onRerun: () => void;
		running: boolean;
	}

	let { summary, system, onRerun, running }: Props = $props();

	const total = $derived(summary.pass + summary.warn + summary.fail);
	const healthPct = $derived(total > 0 ? Math.round((summary.pass / total) * 100) : 0);
</script>

<div class="rounded-lg border border-border bg-muted/20 p-4">
	<div class="flex items-center justify-between gap-4">
		<!-- System info -->
		<div class="min-w-0">
			<div class="flex items-center gap-2">
				<span class="font-semibold">{system.hostname}</span>
				{#if system.bazzite_image}
					<span class="rounded bg-accent px-1.5 py-0.5 font-mono text-xs text-accent-foreground">
						bazzite {system.bazzite_image}
					</span>
				{/if}
			</div>
			<p class="mt-0.5 truncate font-mono text-xs text-muted-foreground">
				{system.kernel}
				{#if system.nvidia_driver}
					· nvidia {system.nvidia_driver}
				{/if}
			</p>
			<p class="mt-0.5 text-xs text-muted-foreground">
				{new Date(system.generated_at).toLocaleString()}
			</p>
		</div>

		<!-- Counts -->
		<div class="flex shrink-0 items-center gap-4">
			<div class="text-center">
				<div class="text-xl font-bold text-status-pass">{summary.pass}</div>
				<div class="text-xs text-muted-foreground">PASS</div>
			</div>
			<div class="text-center">
				<div class={cn('text-xl font-bold', summary.warn > 0 ? 'text-status-warn' : 'text-muted-foreground')}>
					{summary.warn}
				</div>
				<div class="text-xs text-muted-foreground">WARN</div>
			</div>
			<div class="text-center">
				<div class={cn('text-xl font-bold', summary.fail > 0 ? 'text-status-fail' : 'text-muted-foreground')}>
					{summary.fail}
				</div>
				<div class="text-xs text-muted-foreground">FAIL</div>
			</div>

			<!-- Health bar -->
			<div class="w-20">
				<div class="mb-1 text-center text-xs text-muted-foreground">{healthPct}% OK</div>
				<div class="h-2 w-full overflow-hidden rounded-full bg-muted">
					<div
						class="h-full rounded-full bg-status-pass transition-all duration-500"
						style="width: {healthPct}%"
					></div>
				</div>
			</div>

			<!-- Re-run button -->
			<button
				class={cn(
					'rounded-md border border-border px-3 py-1.5 text-sm font-medium',
					'text-muted-foreground hover:border-primary/50 hover:text-primary',
					'transition-colors duration-150',
					running && 'cursor-wait opacity-60'
				)}
				disabled={running}
				onclick={onRerun}
			>
				{#if running}
					<span class="inline-flex items-center gap-1">
						<span class="animate-spin">⟳</span>
						Running…
					</span>
				{:else}
					⟳ Re-run
				{/if}
			</button>
		</div>
	</div>
</div>
