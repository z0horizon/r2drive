<script lang="ts">
  import { bucketStore } from '$lib/stores/bucket.svelte';
  import { HardDrive, ChevronRight, Folder } from 'lucide-svelte';

  function handleNavigate(prefix: string) {
    if (prefix === bucketStore.currentPrefix) return;
    bucketStore.setPrefix(prefix);
  }
</script>

<nav aria-label="Breadcrumb" class="flex items-center overflow-x-auto py-1 text-xs text-slate-300">
  <ol class="flex items-center space-x-1 sm:space-x-1.5 flex-nowrap min-w-0">
    {#each bucketStore.breadcrumbs as crumb, idx (crumb.prefix || 'root')}
      {@const isLast = idx === bucketStore.breadcrumbs.length - 1}
      {@const isRoot = idx === 0}

      {#if idx > 0}
        <li class="shrink-0 text-slate-600" aria-hidden="true">
          <ChevronRight class="w-3.5 h-3.5" />
        </li>
      {/if}

      <li class="flex items-center shrink-0">
        {#if isLast}
          <span
            class="flex items-center gap-1.5 px-2 py-1 rounded-md bg-slate-800 text-white font-medium border border-slate-700/80 shadow-sm"
            aria-current="page"
          >
            {#if isRoot}
              <HardDrive class="w-3.5 h-3.5 text-indigo-400 shrink-0" />
              <span>{bucketStore.selectedProfile || 'Root'}</span>
            {:else}
              <Folder class="w-3.5 h-3.5 text-amber-400 shrink-0" />
              <span>{crumb.label}</span>
            {/if}
          </span>
        {:else}
          <button
            type="button"
            onclick={() => handleNavigate(crumb.prefix)}
            class="flex items-center gap-1.5 px-2 py-1 rounded-md text-slate-400 hover:text-slate-100 hover:bg-slate-800/60 transition"
          >
            {#if isRoot}
              <HardDrive class="w-3.5 h-3.5 text-indigo-400 shrink-0" />
              <span>{bucketStore.selectedProfile || 'Root'}</span>
            {:else}
              <Folder class="w-3.5 h-3.5 text-amber-400/80 shrink-0" />
              <span>{crumb.label}</span>
            {/if}
          </button>
        {/if}
      </li>
    {/each}
  </ol>
</nav>
