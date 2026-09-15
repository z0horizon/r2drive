<script lang="ts">
  import { uploadStore } from '$lib/stores/upload.svelte';
  import { bucketStore } from '$lib/stores/bucket.svelte';
  import { cancelUpload } from '$lib/upload/worker';
  import { formatBytes } from '$lib/utils/format';
  import ProgressBar from './ProgressBar.svelte';
  import {
    Upload,
    ChevronDown,
    ChevronUp,
    X,
    CheckCircle2,
    AlertCircle,
    FileText,
    Trash2,
  } from 'lucide-svelte';

  let isMinimized = $state(false);

  function toggleMinimize() {
    isMinimized = !isMinimized;
  }

  function handleClose() {
    if (uploadStore.activeUploads.length === 0) {
      uploadStore.clearAll();
    } else {
      isMinimized = true;
    }
  }

  async function handleCancel(id: string) {
    await cancelUpload(id);
  }

  function handleRemove(id: string) {
    uploadStore.removeItem(id);
  }

  function handleClearCompleted() {
    uploadStore.clearCompleted();
  }
</script>

{#if uploadStore.items.length > 0}
  <div
    class="fixed bottom-4 right-4 z-40 w-84 sm:w-96 rounded-xl shadow-2xl border border-slate-700/80 bg-slate-900/95 backdrop-blur-md text-slate-100 overflow-hidden font-sans transition-all duration-200"
    role="region"
    aria-label="Upload Transfers"
  >
    <!-- Modal Header -->
    <div class="px-3.5 py-2.5 bg-slate-950/80 border-b border-slate-800 flex items-center justify-between">
      <div class="flex items-center space-x-2 min-w-0">
        {#if uploadStore.isUploading}
          <div class="p-1 bg-indigo-500/20 text-indigo-400 rounded-md">
            <Upload class="w-4 h-4 animate-bounce" />
          </div>
          <div class="min-w-0">
            <p class="text-xs font-semibold text-white truncate">
              Uploading {uploadStore.activeUploads.length} file{uploadStore.activeUploads.length === 1 ? '' : 's'}
            </p>
            <p class="text-[10px] text-slate-400 font-mono">
              {uploadStore.overallProgress}% overall
            </p>
          </div>
        {:else if uploadStore.failedUploads.length > 0}
          <div class="p-1 bg-rose-500/20 text-rose-400 rounded-md">
            <AlertCircle class="w-4 h-4" />
          </div>
          <p class="text-xs font-semibold text-rose-300 truncate">
            {uploadStore.failedUploads.length} transfer{uploadStore.failedUploads.length === 1 ? '' : 's'} failed
          </p>
        {:else}
          <div class="p-1 bg-emerald-500/20 text-emerald-400 rounded-md">
            <CheckCircle2 class="w-4 h-4" />
          </div>
          <p class="text-xs font-semibold text-emerald-300 truncate">
            {uploadStore.completedUploads.length} upload{uploadStore.completedUploads.length === 1 ? '' : 's'} completed
          </p>
        {/if}
      </div>

      <!-- Controls -->
      <div class="flex items-center space-x-1 shrink-0">
        <button
          type="button"
          onclick={toggleMinimize}
          title={isMinimized ? 'Expand panel' : 'Minimize panel'}
          class="p-1 text-slate-400 hover:text-slate-200 rounded-md hover:bg-slate-800 transition"
        >
          {#if isMinimized}
            <ChevronUp class="w-4 h-4" />
          {:else}
            <ChevronDown class="w-4 h-4" />
          {/if}
        </button>

        <button
          type="button"
          onclick={handleClose}
          title={uploadStore.activeUploads.length === 0 ? 'Dismiss all' : 'Minimize'}
          class="p-1 text-slate-400 hover:text-slate-200 rounded-md hover:bg-slate-800 transition"
        >
          <X class="w-4 h-4" />
        </button>
      </div>
    </div>

    <!-- Active Overall Progress Indicator when Minimized -->
    {#if isMinimized && uploadStore.isUploading}
      <div class="h-1 w-full bg-slate-800">
        <div
          class="h-full bg-indigo-500 transition-all duration-300"
          style={`width: ${uploadStore.overallProgress}%`}
        ></div>
      </div>
    {/if}

    <!-- Transfer List (Expanded Body) -->
    {#if !isMinimized}
      <div class="max-h-72 sm:max-h-80 overflow-y-auto divide-y divide-slate-800/80">
        {#each uploadStore.items as item (item.id)}
          <div class="p-3 space-y-2 hover:bg-slate-800/30 transition">
            <!-- File Info & Cancel / Dismiss Action -->
            <div class="flex items-start justify-between gap-2">
              <div class="flex items-center space-x-2 min-w-0">
                <FileText class="w-4 h-4 text-slate-400 shrink-0 mt-0.5" />
                <div class="min-w-0">
                  <div class="flex items-center space-x-1.5 min-w-0">
                    <p class="text-xs font-medium text-slate-200 truncate" title={item.name}>
                      {item.name}
                    </p>
                    {#if item.fallback}
                      <span
                        class="text-xs px-1.5 py-0.5 rounded bg-amber-500/20 text-amber-300 border border-amber-500/30 font-medium shrink-0"
                        title="Uploaded via server proxy (CORS not configured on bucket)"
                      >
                        Proxy fallback
                      </span>
                    {/if}
                  </div>
                  <p class="text-[10px] text-slate-400 font-mono">
                    {formatBytes(item.totalBytes)} • {item.key}
                  </p>
                </div>
              </div>

              <!-- Action Button -->
              {#if item.status === 'uploading' || item.status === 'queued'}
                <button
                  type="button"
                  onclick={() => handleCancel(item.id)}
                  title="Cancel upload"
                  class="p-1 text-slate-400 hover:text-rose-400 hover:bg-rose-500/10 rounded transition shrink-0"
                >
                  <X class="w-3.5 h-3.5" />
                </button>
              {:else}
                <button
                  type="button"
                  onclick={() => handleRemove(item.id)}
                  title="Remove from list"
                  class="p-1 text-slate-400 hover:text-slate-200 hover:bg-slate-800 rounded transition shrink-0"
                >
                  <X class="w-3.5 h-3.5" />
                </button>
              {/if}
            </div>

            <!-- Transfer Progress Bar -->
            <ProgressBar
              progress={item.progress}
              uploadedBytes={item.uploadedBytes}
              totalBytes={item.totalBytes}
              speed={item.speed}
              status={item.status}
              error={item.error}
            />
          </div>
        {/each}
      </div>

      <!-- Fallback Preference Bar -->
      <div class="px-3 py-1.5 bg-slate-950/90 border-t border-slate-800 flex items-center justify-between text-xs text-slate-400">
        <label
          class="flex items-center space-x-2 select-none {bucketStore.serverFallbackPolicy.enabled ? 'cursor-pointer hover:text-slate-300' : 'opacity-50 cursor-not-allowed'}"
          title={bucketStore.serverFallbackPolicy.enabled
            ? 'Automatically route uploads through server proxy if Cloudflare R2 CORS is not configured'
            : 'Proxy upload fallback is disabled by server configuration (config.yaml)'}
        >
          <input
            type="checkbox"
            checked={uploadStore.proxyFallbackPreference && bucketStore.serverFallbackPolicy.enabled}
            disabled={!bucketStore.serverFallbackPolicy.enabled}
            onchange={(e) => uploadStore.setProxyFallbackPreference(e.currentTarget.checked)}
            class="rounded border-slate-700 bg-slate-800 text-indigo-600 focus:ring-0 focus:ring-offset-0 w-3.5 h-3.5 disabled:opacity-40"
          />
          <span class="text-[11px]">Proxy upload fallback</span>
        </label>
        {#if !bucketStore.serverFallbackPolicy.enabled}
          <span class="text-[10px] text-amber-400/80 font-mono">Server disabled</span>
        {/if}
      </div>

      <!-- Modal Footer -->
      <div class="px-3 py-2 bg-slate-950/70 border-t border-slate-800 flex items-center justify-between text-xs text-slate-400">
        <span>
          {uploadStore.completedUploads.length}/{uploadStore.items.length} completed
        </span>

        <button
          type="button"
          onclick={handleClearCompleted}
          disabled={uploadStore.completedUploads.length === 0}
          class="inline-flex items-center space-x-1 px-2 py-1 rounded text-[11px] font-medium text-slate-300 hover:text-white hover:bg-slate-800 transition disabled:opacity-40 disabled:pointer-events-none"
        >
          <Trash2 class="w-3 h-3" />
          <span>Clear completed</span>
        </button>
      </div>
    {/if}
  </div>
{/if}
