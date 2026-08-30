<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import Button from './Button.svelte';

  let {
    icon = 'Circle',
    title,
    description,
    actionLabel,
    actionHref
  }: {
    icon?: string;
    title: string;
    description?: string;
    actionLabel?: string;
    actionHref?: string;
  } = $props();
</script>

<div class="flex flex-col items-center justify-center py-16 px-4 text-center animate-fade-in">
  <div class="mb-4 text-text-muted">
    <Icon name={icon} size={48} />
  </div>
  <h3 class="mb-2 text-xl font-semibold text-text-primary">{title}</h3>
  {#if description}
    <p class="mb-6 max-w-md text-sm text-text-muted">{description}</p>
  {/if}
  {#if actionLabel && actionHref}
    <Button size="md" onclick={() => window.location.href = actionHref}>
      {actionLabel}
    </Button>
  {/if}
</div>
