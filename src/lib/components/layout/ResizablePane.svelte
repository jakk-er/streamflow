<script lang="ts">
  let {
    minWidth = 200,
    maxWidth = 400,
    defaultWidth = 280,
    side = 'left',
    children,
  }: {
    minWidth?: number;
    maxWidth?: number;
    defaultWidth?: number;
    side?: 'left' | 'right';
    children?: import('svelte').Snippet;
  } = $props();

  let width = $state((() => defaultWidth)());
  let isResizing = $state(false);
  let containerRef: HTMLDivElement | null = $state(null);

  const storageKey = 'sidebar-width';

  function loadWidth() {
    if (typeof window !== 'undefined') {
      const stored = localStorage.getItem(storageKey);
      if (stored) {
        const parsed = parseInt(stored, 10);
        if (!isNaN(parsed) && parsed >= minWidth && parsed <= maxWidth) {
          width = parsed;
        }
      }
    }
  }

  function saveWidth() {
    if (typeof window !== 'undefined') {
      localStorage.setItem(storageKey, String(width));
    }
  }

  function handleMouseDown(event: MouseEvent) {
    isResizing = true;
    event.preventDefault();
  }

  function handleMouseMove(event: MouseEvent) {
    if (!isResizing) return;
    const clientX = event.clientX;
    if (side === 'left') {
      width = Math.min(maxWidth, Math.max(minWidth, clientX));
    } else {
      const viewportWidth = window.innerWidth;
      width = Math.min(maxWidth, Math.max(minWidth, viewportWidth - clientX));
    }
  }

  function handleMouseUp() {
    if (isResizing) {
      isResizing = false;
      saveWidth();
    }
  }

  $effect(() => {
    loadWidth();
    if (isResizing) {
      window.addEventListener('mousemove', handleMouseMove);
      window.addEventListener('mouseup', handleMouseUp);
    }
    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  });

  let handleClass = $derived(side === 'left' ? 'right-0' : 'left-0');
  const cursorClass = 'cursor-col-resize';
</script>

<div
  bind:this={containerRef}
  class="relative flex-shrink-0"
  style="width: {width}px"
>
  {@render children?.()}
  <button
    type="button"
    class={`absolute top-0 bottom-0 w-1 ${handleClass} ${cursorClass} hover:bg-blue-500/50 active:bg-blue-500/80 transition-colors`}
    onmousedown={handleMouseDown}
    aria-label="Resize sidebar"
  ></button>
</div>
