<script lang="ts">
	import { cn } from '$lib/utils';

	interface Props {
		checkName: string;
		fixCommand: string;
		fixing: string | null;
		onFix: (checkName: string, fixCommand: string) => void;
	}

	let { checkName, fixCommand, fixing, onFix }: Props = $props();

	const isFixingThis = $derived(fixing === checkName);
	const isFixingOther = $derived(fixing !== null && fixing !== checkName);
</script>

<button
	class={cn(
		'shrink-0 rounded-md border px-3 py-1.5 text-xs font-medium transition-all duration-150',
		'border-status-fail/40 text-status-fail hover:bg-status-fail/10',
		isFixingThis && 'cursor-wait border-status-warn/40 text-status-warn',
		isFixingOther && 'cursor-not-allowed opacity-40'
	)}
	disabled={fixing !== null}
	onclick={() => onFix(checkName, fixCommand)}
>
	{#if isFixingThis}
		<span class="inline-flex items-center gap-1">
			<span class="animate-spin">⟳</span>
			Fixing…
		</span>
	{:else}
		Fix it
	{/if}
</button>
