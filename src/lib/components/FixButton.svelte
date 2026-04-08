<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!--
  FixButton.svelte — Session 6
  Two-step confirmation guard for destructive fix commands.

  Commands containing "restart plasma-kwin_wayland" terminate the active
  Wayland compositor and end the session. First click shows an amber warning
  with confirm/cancel. All other commands execute immediately.
-->
<script lang="ts">
	import { cn } from '$lib/utils';

	interface Props {
		checkName: string;
		fixCommand: string;
		fixing: string | null;
		onFix: (checkName: string, fixCommand: string) => void;
	}

	let { checkName, fixCommand, fixing, onFix }: Props = $props();

	const isFixingThis  = $derived(fixing === checkName);
	const isFixingOther = $derived(fixing !== null && fixing !== checkName);

	// Commands that terminate the Wayland session require explicit confirmation.
	const DESTRUCTIVE_PATTERNS = ['restart plasma-kwin_wayland', 'restart kwin_wayland'];
	const isDestructive = $derived(
		DESTRUCTIVE_PATTERNS.some((p) => fixCommand.includes(p))
	);

	let pendingConfirm = $state(false);

	function handleClick() {
		if (isDestructive && !pendingConfirm) {
			pendingConfirm = true;
			return;
		}
		pendingConfirm = false;
		onFix(checkName, fixCommand);
	}

	function cancelConfirm() {
		pendingConfirm = false;
	}
</script>

{#if pendingConfirm}
	<div class="flex shrink-0 flex-col gap-1.5 rounded-md border border-status-warn/50 bg-status-warn/10 p-2.5 text-xs">
		<p class="font-medium text-status-warn leading-snug">
			⚠ This will restart your compositor and <strong>end your Wayland session</strong>.
			You will need to log back in.
		</p>
		<div class="flex gap-1.5">
			<button
				class="rounded border border-status-fail/40 px-2.5 py-1 text-status-fail hover:bg-status-fail/10 transition-colors disabled:opacity-40"
				onclick={handleClick}
				disabled={fixing !== null}
			>
				Yes, restart compositor
			</button>
			<button
				class="rounded border border-border px-2.5 py-1 text-muted-foreground hover:text-foreground transition-colors"
				onclick={cancelConfirm}
			>
				Cancel
			</button>
		</div>
	</div>
{:else}
	<button
		class={cn(
			'shrink-0 rounded-md border px-3 py-1.5 text-xs font-medium transition-all duration-150',
			'border-status-fail/40 text-status-fail hover:bg-status-fail/10',
			isFixingThis  && 'cursor-wait border-status-warn/40 text-status-warn',
			isFixingOther && 'cursor-not-allowed opacity-40'
		)}
		disabled={fixing !== null}
		onclick={handleClick}
	>
		{#if isFixingThis}
			<span class="inline-flex items-center gap-1">
				<span class="animate-spin">⟳</span>
				Fixing…
			</span>
		{:else if isDestructive}
			Fix it ⚠
		{:else}
			Fix it
		{/if}
	</button>
{/if}
