<script lang="ts">
  import { bucketStore } from '$lib/stores/bucket.svelte';
  import { getDownloadUrl, type ObjectItem } from '$lib/api/objects';
  import { formatBytes, formatDate } from '$lib/utils/format';
  import { File, Download, Trash2, RefreshCw } from 'lucide-svelte';

  let {
    object,
    onDelete,
  }: {
    object: ObjectItem;
    onDelete?: (object: ObjectItem) => void;
  } = $props();

  let isDownloading = $state(false);

  async function handleDownload(e: MouseEvent) {
    e.stopPropagation();
    if (isDownloading) return;

    isDownloading = true;
    try {
      const downloadUrl = await getDownloadUrl(bucketStore.selectedProfile, object.key);
      const link = document.createElement('a');
      link.href = downloadUrl;
      link.download = object.name || object.key.split('/').pop() || 'download';
      link.target = '_blank';
      link.rel = 'noopener noreferrer';
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
    } catch (err) {
      console.error('Download failed:', err);
      alert('Failed to get download URL: ' + (err instanceof Error ? err.message : String(err)));
    } finally {
      isDownloading = false;
    }
  }

  function handleDelete(e: MouseEvent) {
    e.stopPropagation();
    onDelete?.(object);
  }
</script>

<tr class="group border-b border-slate-800/60 hover:bg-slate-800/30 transition">
  <td class="py-3 px-4 min-w-0">
    <div class="flex items-center gap-3">
      <File class="w-4 h-4 text-slate-400 shrink-0 group-hover:text-slate-300 transition-colors" />
      <div class="min-w-0 flex-1">
        <div class="text-sm font-medium text-slate-200 truncate" title={object.key}>
          {object.name || object.key.split('/').pop() || object.key}
        </div>
      </div>
    </div>
  </td>
  <td class="py-3 px-4 text-xs text-slate-400 font-mono text-right whitespace-nowrap">
    {formatBytes(object.size_bytes)}
  </td>
  <td class="py-3 px-4 text-xs text-slate-400 text-right whitespace-nowrap hidden sm:table-cell">
    {formatDate(object.last_modified)}
  </td>
  <td class="py-3 px-4 text-right whitespace-nowrap">
    <div class="flex items-center justify-end gap-1">
      <button
        type="button"
        onclick={handleDownload}
        disabled={isDownloading}
        title="Download file"
        class="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-700/60 transition disabled:opacity-50"
      >
        {#if isDownloading}
          <RefreshCw class="w-4 h-4 animate-spin text-indigo-400" />
        {:else}
          <Download class="w-4 h-4" />
        {/if}
      </button>
      <button
        type="button"
        onclick={handleDelete}
        title="Delete file"
        class="p-1.5 rounded-lg text-slate-400 hover:text-rose-400 hover:bg-rose-950/40 transition"
      >
        <Trash2 class="w-4 h-4" />
      </button>
    </div>
  </td>
</tr>
