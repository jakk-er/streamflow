<script lang="ts">
  let {
    checked = false,
    onchange,
    ariaLabel = 'Toggle switch'
  }: {
    checked?: boolean;
    onchange?: (checked: boolean) => void;
    ariaLabel?: string;
  } = $props();

  function handleKeyDown(event: KeyboardEvent) {
    if (event.key === ' ' || event.key === 'Enter') {
      event.preventDefault();
      onchange?.(!checked);
    }
  }
</script>

<button
  role="switch"
  aria-checked={checked}
  aria-label={ariaLabel}
  tabindex="0"
  class="relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full transition-colors duration-200 ease-out {checked ? 'bg-primary' : 'bg-border'}"
  onclick={() => onchange?.(!checked)}
  onkeydown={handleKeyDown}
>
  <span
    class="pointer-events-none inline-block h-5 w-5 rounded-full bg-white shadow-md transition-transform duration-200 ease-out {checked ? 'translate-x-5' : 'translate-x-0.5'}"
  ></span>
</button>
