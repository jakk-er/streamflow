<script lang="ts">
  let {
    value = $bindable(''),
    placeholder = '',
    type = 'text',
    label,
    id,
    oninput,
    class: className = ''
  }: {
    value?: string;
    placeholder?: string;
    type?: string;
    label?: string;
    id?: string;
    oninput?: (event: Event) => void;
    class?: string;
  } = $props();

  let inputId = $derived(id || `input-${Math.random().toString(36).slice(2, 9)}`);
</script>

{#if label}
  <label for={inputId} class="block text-sm font-medium text-text-secondary mb-1.5">
    {label}
  </label>
{/if}

<input
  id={inputId}
  {type}
  {placeholder}
  bind:value
  {oninput}
  class={`input ${className}`}
/>
