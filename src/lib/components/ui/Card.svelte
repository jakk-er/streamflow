<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    children,
    class: className = '',
    hover = true
  }: {
    children: Snippet;
    class?: string;
    hover?: boolean;
  } = $props();

  const hoverClass = $derived(hover ? 'card-hover group' : '');
</script>

<div class={`card ${hoverClass} ${className}`}>
  {@render children?.()}
</div>
