<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    variant = 'default',
    children,
    class: className = ''
  }: {
    variant?: 'default' | 'primary' | 'accent' | 'success';
    children: Snippet;
    class?: string;
  } = $props();

  const variantClasses: Record<string, string> = {
    default: 'badge-default',
    primary: 'badge-primary',
    accent: 'badge-accent',
    success: 'badge-success'
  };
</script>

<span class={`badge ${variantClasses[variant]} ${className}`}>
  {@render children?.()}
</span>
