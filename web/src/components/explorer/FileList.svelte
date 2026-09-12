<script lang="ts">
  import { bucketStore } from '$lib/stores/bucket.svelte';
  import type { ObjectItem } from '$lib/api/objects';
  import FolderRow from './FolderRow.svelte';
  import FileRow from './FileRow.svelte';
  import EmptyState from './EmptyState.svelte';
  import DeleteModal from '../modals/DeleteModal.svelte';
  import { RefreshCw } from 'lucide-svelte';

  let itemToDelete = $state<ObjectItem | null>(null);
  let showDeleteModal = $state(false);

  function handleDeleteClick(item: ObjectItem) {
    itemToDelete = item;
    showDeleteModal = true;
  }

  function handleCloseDeleteModal() {
    showDeleteModal = false;
    itemToDelete = null;
  }

  const isEmpty = $derived(
    bucketStore.directories.length === 0 && bucketStore.objects.length === 0
  );
</script>

<div class="space-y-4">
  {#if bucketStore.loading && isEmpty}
    <!-- Initial Loading State -->
    <div class="border border-slate-800/80 rounded-2xl p-14 text-center flex flex-col items-center justify-center space-y-3 bg-slate-950/40 my-4">
      <RefreshCw class="w-8 h-8 animate-spin text-indigo-400" />
      <p class="text-xs text-slate-400 font-mono">Fetching bucket contents...</p>
    </div>
  {:else if !bucketStore.loading && isEmpty}
    <!-- Empty State -->
    <EmptyState
      title="No files or folders here"
      message="This prefix contains no objects in the selected bucket."
    />
  {:else}
    <!-- File & Folder Listing Table -->
    <div class="relative overflow-hidden rounded-xl border border-slate-800 bg-slate-950/40 shadow-sm">
      {#if bucketStore.loading}
        <div class="absolute inset-x-0 top-0 h-0.5 bg-indigo-500/20 overflow-hidden z-10">
          <div class="h-full bg-indigo-500 w-1/3 animate-[pulse_1s_ease-in-out_infinite]"></div>
        </div>
      {/if}

      <div class="overflow-x-auto">
        <table class="w-full text-left border-collapse">
          <thead>
            <tr class="border-b border-slate-800 bg-slate-900/80 text-[11px] font-semibold text-slate-400 uppercase tracking-wider select-none">
              <th scope="col" class="py-3 px-4">Name</th>
              <th scope="col" class="py-3 px-4 text-right w-28">Size</th>
              <th scope="col" class="py-3 px-4 text-right w-44 hidden sm:table-cell">Last Modified</th>
              <th scope="col" class="py-3 px-4 text-right w-28">Actions</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-800/40">
            {#each bucketStore.directories as dirPath (dirPath)}
              <FolderRow {dirPath} />
            {/each}

            {#each bucketStore.objects as object (object.key)}
              <FileRow {object} onDelete={handleDeleteClick} />
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {/if}

  <!-- Delete Confirmation Modal -->
  <DeleteModal
    open={showDeleteModal}
    item={itemToDelete}
    onClose={handleCloseDeleteModal}
  />
</div>
