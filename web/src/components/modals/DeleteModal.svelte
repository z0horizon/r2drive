<script lang="ts">
  import { bucketStore } from '$lib/stores/bucket.svelte';
  import type { ObjectItem } from '$lib/api/objects';
  import { AlertTriangle, Trash2, RefreshCw, X } from 'lucide-svelte';

  let {
    open = false,
    item = null,
    onClose,
    onDeleted,
  }: {
    open: boolean;
    item: ObjectItem | { key: string; name?: string } | null;
    onClose: () => void;
    onDeleted?: (key: string) => void;
  } = $props();

  let isDeleting = $state(false);
  let errorMessage = $state<string | null>(null);

  async function handleConfirm() {
    if (!item || isDeleting) return;

    isDeleting = true;
    errorMessage = null;

    try {
      await bucketStore.deleteItem(item.key);
      onDeleted?.(item.key);
      onClose();
    } catch (err) {
      errorMessage = err instanceof Error ? err.message : 'Failed to delete object';
    } finally {
      isDeleting = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !isDeleting) {
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open && item}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm"
    role="dialog"
    aria-modal="true"
    aria-labelledby="delete-dialog-title"
  >
    <div class="w-full max-w-md rounded-2xl border border-slate-800 bg-slate-900 p-6 shadow-2xl space-y-5">
      <div class="flex items-start justify-between gap-3">
        <div class="flex items-center gap-3">
          <div class="flex items-center justify-center w-10 h-10 rounded-xl bg-rose-500/10 border border-rose-500/20 text-rose-400 shrink-0">
            <Trash2 class="w-5 h-5" />
          </div>
          <div>
            <h3 id="delete-dialog-title" class="text-base font-semibold text-white">
              Delete Object
            </h3>
            <p class="text-xs text-slate-400">This operation is permanent</p>
          </div>
        </div>
        <button
          type="button"
          onclick={onClose}
          disabled={isDeleting}
          class="p-1 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition disabled:opacity-50"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <div class="text-xs text-slate-300 space-y-2 bg-slate-950/40 p-3.5 rounded-xl border border-slate-800">
        <p>Are you sure you want to permanently delete this object from Cloudflare R2?</p>
        <p class="font-mono text-slate-100 break-all bg-slate-900 px-2.5 py-1.5 rounded border border-slate-800">
          {item.name || item.key}
        </p>
      </div>

      {#if errorMessage}
        <div class="flex items-start gap-2.5 rounded-lg border border-rose-500/30 bg-rose-500/10 p-3 text-xs text-rose-300">
          <AlertTriangle class="w-4 h-4 shrink-0 text-rose-400 mt-0.5" />
          <span>{errorMessage}</span>
        </div>
      {/if}

      <div class="flex items-center justify-end gap-3 pt-2">
        <button
          type="button"
          onclick={onClose}
          disabled={isDeleting}
          class="px-4 py-2 rounded-lg text-xs font-medium text-slate-300 hover:bg-slate-800 hover:text-white border border-slate-700 transition disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          type="button"
          onclick={handleConfirm}
          disabled={isDeleting}
          class="inline-flex items-center gap-1.5 px-4 py-2 rounded-lg text-xs font-medium text-white bg-rose-600 hover:bg-rose-500 transition shadow-sm disabled:opacity-50"
        >
          {#if isDeleting}
            <RefreshCw class="w-3.5 h-3.5 animate-spin" />
            <span>Deleting...</span>
          {:else}
            <Trash2 class="w-3.5 h-3.5" />
            <span>Delete</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}
