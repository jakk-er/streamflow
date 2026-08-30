<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Icon from '$lib/components/ui/Icon.svelte';
  import MarqueeText from '$lib/components/ui/MarqueeText.svelte';

  // Custom dropdown, not native `<select>` - a native select's text is browser-rendered
  // and can't be measured/animated, but we need the selected label to marquee-scroll.
  let {
    options,
    value,
    onchange,
    ariaLabel,
    placeholder = 'Select…',
    class: className = '',
  }: {
    options: { value: string; label: string }[];
    value: string;
    onchange: (value: string) => void;
    ariaLabel?: string;
    placeholder?: string;
    class?: string;
  } = $props();

  let open = $state(false);
  let rootRef: HTMLDivElement | null = $state(null);
  let listRef: HTMLUListElement | null = $state(null);

  let selectedLabel = $derived(options.find((o) => o.value === value)?.label ?? placeholder);

  onMount(() => {
    function handlePointerDown(event: PointerEvent) {
      if (open && rootRef && !rootRef.contains(event.target as Node)) {
        open = false;
      }
    }
    window.addEventListener('pointerdown', handlePointerDown);
    return () => window.removeEventListener('pointerdown', handlePointerDown);
  });

  // Scrolls to the selected option on open; `tick()` waits for the list to exist in the
  // DOM, and `block: 'nearest'` avoids re-centering if it's already in view.
  async function toggleOpen() {
    open = !open;
    if (open) {
      await tick();
      listRef?.querySelector('[data-selected="true"]')?.scrollIntoView({ block: 'nearest' });
    }
  }

  function selectOption(optionValue: string) {
    onchange(optionValue);
    open = false;
  }
</script>

<div class="relative {className}" bind:this={rootRef}>
  <button
    type="button"
    class="flex w-full items-center justify-between gap-2 rounded-lg border border-border bg-surface py-1.5 px-2 text-sm text-text-primary outline-none transition-all duration-200 focus:border-primary focus:ring-1 focus:ring-primary"
    onclick={toggleOpen}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={ariaLabel}
  >
    <MarqueeText text={selectedLabel} mode="always" class="flex-1 text-left" />
    <Icon
      name="ChevronDown"
      size={14}
      class="flex-shrink-0 text-text-muted transition-transform duration-200 {open ? 'rotate-180' : ''}"
    />
  </button>

  {#if open}
    <ul
      bind:this={listRef}
      role="listbox"
      class="absolute z-20 mt-1 max-h-64 w-max min-w-full max-w-xs overflow-y-auto rounded-lg border border-border bg-surface py-1 shadow-lg"
    >
      {#each options as option (option.value)}
        <li>
          <button
            type="button"
            role="option"
            data-selected={option.value === value}
            aria-selected={option.value === value}
            class="flex w-full items-center px-3 py-1.5 text-left text-sm transition-colors duration-150 {option.value ===
            value
              ? 'bg-primary/15 text-primary'
              : 'text-text-primary hover:bg-surface-hover'}"
            onclick={() => selectOption(option.value)}
          >
            <MarqueeText text={option.label} mode="hover" class="flex-1" />
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
