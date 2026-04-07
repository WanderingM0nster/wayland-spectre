<script lang="ts">
	import type { DiagnosticResult, LayerLabel } from '$lib/types';
	import { LAYER_META, STATUS_COLOURS, STATUS_BG } from '$lib/types';
	import { cn } from '$lib/utils';
	import FixButton from './FixButton.svelte';

	interface Props {
		layer: LayerLabel;
		results: DiagnosticResult[];
		fixing: string | null;
		onFix: (checkName: string, fixCommand: string) => void;
	}

	let { layer, results, fixing, onFix }: Props = $props();

	let expanded = $state(false);

	const meta = $derived(LAYER_META[layer]);

	// Worst status across all results in this layer
	const layerStatus = $derived.by(() => {
		if (results.some((r) => r.status === 'FAIL')) return 'FAIL' as const;
		if (results.some((r) => r.status === 'WARN')) return 'WARN' as const;
		if (results.every((r) => r.status === 'SKIP')) return 'SKIP' as const;
		return 'PASS' as const;
	});

	const statusIcon = $derived.by(() => {
		switch (layerStatus) {
			case 'PASS': return '✓';
			case 'WARN': return '⚠';
			case 'FAIL': return '✗';
			case 'SKIP': return '–';
		}
	});
</script>

<div class={cn('rounded-lg border transition-all duration-200', STATUS_BG[layerStatus])}>
	<!-- Layer header row -->
	<button
		class="flex w-full items-center gap-3 px-4 py-3 text-left"
		onclick={() => (expanded = !expanded)}
		aria-expanded={expanded}
	>
		<!-- Status badge -->
		<span
			class={cn(
				'flex h-8 w-8 shrink-0 items-center justify-center rounded-md font-mono text-sm font-bold',
				STATUS_COLOURS[layerStatus]
			)}
		>
			{statusIcon}
		</span>

		<!-- Layer info -->
		<div class="min-w-0 flex-1">
			<div class="flex items-center gap-2">
				<span class="font-mono text-xs text-muted-foreground">{layer}</span>
				<span class="font-semibold">{meta.name}</span>
			</div>
			<p class="truncate text-xs text-muted-foreground">{meta.description}</p>
		</div>

		<!-- Check count -->
		<span class="shrink-0 text-xs text-muted-foreground">
			{results.length} check{results.length !== 1 ? 's' : ''}
		</span>

		<!-- Expand chevron -->
		<span
			class={cn(
				'shrink-0 text-muted-foreground transition-transform duration-200',
				expanded && 'rotate-180'
			)}
		>
			▾
		</span>
	</button>

	<!-- Expanded results -->
	{#if expanded}
		<div class="border-t border-inherit px-4 pb-3 pt-2">
			<div class="space-y-2">
				{#each results as result (result.check)}
					<div class="rounded-md bg-background/40 p-3">
						<div class="flex items-start justify-between gap-3">
							<div class="min-w-0 flex-1">
								<div class="flex items-center gap-2">
									<span class={cn('text-xs font-bold', STATUS_COLOURS[result.status])}>
										{result.status}
									</span>
									<span class="font-mono text-sm">{result.check}</span>
									<span class="text-xs text-muted-foreground">
										({result.confidence})
									</span>
								</div>
								<p class="mt-1 text-sm text-muted-foreground">{result.detail}</p>
								{#if result.fix}
									<code class="mt-1 block text-xs text-muted-foreground/70">
										$ {result.fix}
									</code>
								{/if}
							</div>

							{#if result.fix && result.status !== 'PASS' && result.status !== 'SKIP'}
								<FixButton
									checkName={result.check}
									fixCommand={result.fix}
									{fixing}
									{onFix}
								/>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		</div>
	{/if}
</div>
