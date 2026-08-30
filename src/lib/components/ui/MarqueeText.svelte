<script lang="ts">
  // `mode="always"` for MarqueeSelect's trigger; `mode="hover"` for option rows so
  // measuring happens lazily per-row instead of upfront for the whole list.
  let {
    text,
    mode = 'always',
    class: className = '',
  }: {
    text: string;
    mode?: 'always' | 'hover';
    class?: string;
  } = $props();

  let wrapperRef: HTMLSpanElement | null = $state(null);
  let textRef: HTMLSpanElement | null = $state(null);
  let overflowPx = $state(0);
  let hovering = $state(false);

  function measure() {
    if (!wrapperRef || !textRef) return;
    overflowPx = Math.max(0, textRef.scrollWidth - wrapperRef.clientWidth);
  }

  $effect(() => {
    // Re-measure whenever the displayed text changes - `queueMicrotask` so
    // this runs after Svelte has actually updated the DOM text, not before.
    text;
    queueMicrotask(measure);
  });

  let shouldAnimate = $derived(overflowPx > 0 && (mode === 'always' || hovering));
</script>

<span
  bind:this={wrapperRef}
  class="block min-w-0 overflow-hidden {className}"
  role="presentation"
  onmouseenter={() => {
    if (mode === 'hover') {
      hovering = true;
      measure();
    }
  }}
  onmouseleave={() => {
    if (mode === 'hover') hovering = false;
  }}
>
  <span
    bind:this={textRef}
    class="inline-block whitespace-nowrap"
    class:marquee-text-animate={shouldAnimate}
    style={shouldAnimate ? `--marquee-distance: ${overflowPx}px` : ''}
  >
    {text}
  </span>
</span>

<style>
  /* Pause at the start (readable), slide left to reveal the tail, pause
     there (readable), slide back - never a plain continuous ticker, since
     this represents a single value/label, not a scrolling feed. */
  @keyframes marquee-text {
    0%,
    15% {
      transform: translateX(0);
    }
    50%,
    65% {
      transform: translateX(calc(-1 * var(--marquee-distance)));
    }
    100% {
      transform: translateX(0);
    }
  }

  .marquee-text-animate {
    animation: marquee-text 7s ease-in-out infinite;
  }
</style>
