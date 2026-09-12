<script lang="ts">
  import { bucketStore } from '$lib/stores/bucket.svelte';
  import { Folder, ChevronRight } from 'lucide-svelte';

  let {
    dirPath,
    onNavigate,
  }: {
    dirPath: string;
    onNavigate?: (path: string) => void;
  } = $props();

  const folderName = $derived.by(() => {
    const clean = dirPath.endsWith('/') ? dirPath.slice(0, -1) : dirPath;
    const parts = clean.split('/');
    return parts[parts.length - 1] || dirPath;
  });

  function handleClick() {
    if (onNavigate) {
      onNavigate(dirPath);
    } else {
      bucketStore.setPrefix(dirPath);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      handleClick();
    }
  }
</script>

<tr
  tabindex="0"
  role="button"
  onclick={handleClick}
  onkeydown={handleKeydown}
  class="group cursor-pointer border-b border-slate-800/60 hover:bg-slate-800/40 focus:bg-slate-800/50 focus:outline-none transition select-none"
>
  <td class="py-3 px-4 min-w-0">
    <div class="flex items-center gap-3">
      <Folder class="w-4 h-4 text-amber-400 fill-amber-400/20 shrink-0 group-hover:scale-110 transition-transform" />
      <span class="text-sm font-medium text-slate-200 group-hover:text-indigo-300 truncate transition-colors" title={dirPath}>
        {folderName}
      </span>
    </div>
  </td>
  <td class="py-3 px-4 text-xs text-slate-500 font-mono text-right whitespace-nowrap">
    —
  </td>
  <td class="py-3 px-4 text-xs text-slate-500 text-right whitespace-nowrap hidden sm:table-cell">
    —
  </td>
  <td class="py-3 px-4 text-right whitespace-nowrap">
    <span class="inline-flex items-center gap-1 text-xs text-slate-500 group-hover:text-indigo-300 transition">
      <span class="hidden md:inline">Open</span>
      <ChevronRight class="w-3.5 h-3.5" />
    </span>
  </td>
</tr>
