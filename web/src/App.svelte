<script lang="ts">
  import { onMount } from 'svelte';
  import { authStore } from '$lib/stores/auth.svelte';
  import { bucketStore } from '$lib/stores/bucket.svelte';
  import Header from './components/layout/Header.svelte';
  import Breadcrumbs from './components/layout/Breadcrumbs.svelte';
  import FileList from './components/explorer/FileList.svelte';
  import LoginModal from './components/modals/LoginModal.svelte';
  import DropZone from './components/upload/DropZone.svelte';
  import UploadModal from './components/upload/UploadModal.svelte';
  import { HardDrive, RefreshCw, Upload, AlertCircle } from 'lucide-svelte';

  let initialized = $state(false);
  let dropZone: ReturnType<typeof DropZone> | null = $state(null);

  onMount(async () => {
    try {
      const isAuthed = await authStore.check();
      if (isAuthed) {
        await bucketStore.loadProfiles();
      }
    } finally {
      initialized = true;
    }
  });

  async function handleRefresh() {
    if (bucketStore.loading) return;
    await bucketStore.refresh();
  }

  function handleUploadClick() {
    dropZone?.openPicker();
  }
</script>

{#if !initialized}
  <!-- Initial startup splash -->
  <div class="min-h-screen bg-slate-900 text-slate-100 flex items-center justify-center font-sans antialiased">
    <div class="flex flex-col items-center space-y-4">
      <div class="p-3 bg-indigo-600/20 text-indigo-400 rounded-xl border border-indigo-500/30">
        <HardDrive class="w-8 h-8 animate-pulse" />
      </div>
      <p class="text-xs text-slate-400 font-mono">Initializing WebConsole...</p>
    </div>
  </div>
{:else}
  <div class="min-h-screen bg-slate-900 text-slate-100 flex flex-col font-sans antialiased">
    <Header />

    {#if !authStore.isAuthenticated}
      <!-- Authentication challenge modal -->
      <LoginModal />
    {:else}
      <!-- Main Content Area -->
      <main class="flex-1 max-w-7xl w-full mx-auto p-4 sm:p-6 space-y-6">
        <!-- Error Alert Banner -->
        {#if bucketStore.error}
          <div class="flex items-start gap-3 rounded-xl border border-rose-500/30 bg-rose-500/10 p-4 text-xs sm:text-sm text-rose-300">
            <AlertCircle class="w-5 h-5 shrink-0 text-rose-400 mt-0.5" />
            <div class="flex-1 min-w-0">
              <p class="font-medium text-rose-200">Error loading bucket</p>
              <p class="mt-0.5 text-xs text-rose-300/90">{bucketStore.error}</p>
            </div>
            <button
              type="button"
              onclick={handleRefresh}
              class="px-2.5 py-1 rounded-md bg-rose-500/20 hover:bg-rose-500/30 text-rose-200 text-xs font-medium border border-rose-500/40 transition shrink-0"
            >
              Retry
            </button>
          </div>
        {/if}

        <!-- Drag & Drop Overlay, Hidden Input & Resume Sessions Banner -->
        <DropZone bind:this={dropZone} />

        <!-- Navigation Breadcrumbs & Actions Toolbar -->
        <div class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 bg-slate-950/40 p-3 sm:p-4 rounded-xl border border-slate-800/80">
          <div class="min-w-0 flex-1 w-full sm:w-auto">
            <Breadcrumbs />
          </div>

          <div class="flex items-center space-x-2 shrink-0 self-end sm:self-auto">
            <button
              type="button"
              onclick={handleRefresh}
              disabled={bucketStore.loading}
              class="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 transition disabled:opacity-50"
            >
              <RefreshCw class={`w-3.5 h-3.5 ${bucketStore.loading ? 'animate-spin text-indigo-400' : ''}`} />
              <span>Refresh</span>
            </button>
            <button
              type="button"
              onclick={handleUploadClick}
              class="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-indigo-600 hover:bg-indigo-500 text-white shadow-sm transition cursor-pointer"
            >
              <Upload class="w-3.5 h-3.5" />
              <span>Upload</span>
            </button>
          </div>
        </div>

        <!-- Directory & File Explorer -->
        <FileList />

        <!-- Floating Upload Manager Panel -->
        <UploadModal />
      </main>
    {/if}
  </div>
{/if}
