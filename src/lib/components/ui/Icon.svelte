<script lang="ts">
  import Home from 'lucide-svelte/icons/home';
  import Play from 'lucide-svelte/icons/play';
  import Pause from 'lucide-svelte/icons/pause';
  import Volume2 from 'lucide-svelte/icons/volume-2';
  import VolumeX from 'lucide-svelte/icons/volume-x';
  import Maximize from 'lucide-svelte/icons/maximize';
  import Minimize from 'lucide-svelte/icons/minimize';
  import Star from 'lucide-svelte/icons/star';
  import Search from 'lucide-svelte/icons/search';
  import Settings from 'lucide-svelte/icons/settings';
  import Tv from 'lucide-svelte/icons/tv';
  import Film from 'lucide-svelte/icons/film';
  import Heart from 'lucide-svelte/icons/heart';
  import History from 'lucide-svelte/icons/history';
  import ChevronLeft from 'lucide-svelte/icons/chevron-left';
  import ChevronRight from 'lucide-svelte/icons/chevron-right';
  import X from 'lucide-svelte/icons/x';
  import Menu from 'lucide-svelte/icons/menu';
  import Plus from 'lucide-svelte/icons/plus';
  import Trash2 from 'lucide-svelte/icons/trash-2';
  import RefreshCw from 'lucide-svelte/icons/refresh-cw';
  import Download from 'lucide-svelte/icons/download';
  import ExternalLink from 'lucide-svelte/icons/external-link';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import Radio from 'lucide-svelte/icons/radio';
  import AlertCircle from 'lucide-svelte/icons/alert-circle';
  import RotateCcw from 'lucide-svelte/icons/rotate-ccw';
  import RotateCw from 'lucide-svelte/icons/rotate-cw';
  import Sun from 'lucide-svelte/icons/sun';
  import Rewind from 'lucide-svelte/icons/rewind';
  import ChevronDown from 'lucide-svelte/icons/chevron-down';
  import ArrowUpAZ from 'lucide-svelte/icons/arrow-up-a-z';
  import ArrowDownAZ from 'lucide-svelte/icons/arrow-down-a-z';
  import SkipBack from 'lucide-svelte/icons/skip-back';
  import Minus from 'lucide-svelte/icons/minus';
  import LayoutList from 'lucide-svelte/icons/layout-list';
  import LayoutGrid from 'lucide-svelte/icons/layout-grid';
  import Folder from 'lucide-svelte/icons/folder';

  let {
    name,
    size = 16,
    class: className = '',
    ...props
  }: {
    name: string;
    size?: number;
    class?: string;
    [key: string]: unknown;
  } = $props();

  const iconMap: Record<string, any> = {
    Home,
    Play,
    Pause,
    Volume2,
    VolumeX,
    Maximize,
    Minimize,
    Star,
    Search,
    Settings,
    Tv,
    Film,
    Heart,
    History,
    ChevronLeft,
    ChevronRight,
    X,
    Menu,
    Plus,
    Trash2,
    RefreshCw,
    Download,
    ExternalLink,
    Loader2,
    Radio,
    AlertCircle,
    RotateCcw,
    RotateCw,
    Sun,
    Rewind,
    ChevronDown,
    ArrowUpAZ,
    ArrowDownAZ,
    SkipBack,
    LayoutList,
    LayoutGrid,
    Folder,
    Minus
  };

  let IconComponent = $derived(iconMap[name] || null);

  let computedClass = $derived(`lucide lucide-${name.toLowerCase()} ${className}`);
</script>

{#if IconComponent}
  <IconComponent {size} class={computedClass} {...props} />
{/if}
