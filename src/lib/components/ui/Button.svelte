<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    variant = 'primary',
    size = 'md',
    disabled = false,
    children,
    onclick,
    'aria-label': ariaLabel,
    class: className = ''
  }: {
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
    size?: 'sm' | 'md' | 'lg';
    disabled?: boolean;
    children: Snippet;
    onclick?: () => void;
    'aria-label'?: string;
    class?: string;
  } = $props();

  const baseClasses = 'btn inline-flex items-center justify-center gap-2 rounded-lg font-medium transition-all duration-200 ease-out focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 focus:ring-offset-background';

  const variantClasses = {
    primary: 'btn-primary shadow-lg shadow-primary/25',
    secondary: 'btn-secondary',
    ghost: 'btn-ghost',
    danger: 'btn-danger'
  };

  const sizeClasses = {
    sm: 'btn-sm',
    md: 'btn-md',
    lg: 'btn-lg'
  };

  const classes = $derived(`${baseClasses} ${variantClasses[variant]} ${sizeClasses[size]} ${disabled ? 'opacity-50 cursor-not-allowed' : 'hover:scale-[1.02] active:scale-[0.98]'} ${className}`);
</script>

<button
  {disabled}
  class={classes}
  onclick={onclick}
  aria-label={ariaLabel}
>
  {@render children?.()}
</button>
