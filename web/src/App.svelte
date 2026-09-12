<script lang="ts">
  import { HardDrive, Folder, Cloud, RefreshCw, Upload } from 'lucide-svelte';

  let profile = $state('primary');
  let prefix = $state('');
</script>

<div class="min-h-screen bg-slate-900 text-slate-100 flex flex-col font-sans antialiased">
  <!-- Top Navigation Header -->
  <header class="border-b border-slate-800 bg-slate-950/70 backdrop-blur px-6 py-4 flex items-center justify-between sticky top-0 z-10">
    <div class="flex items-center space-x-3">
      <div class="p-2 bg-indigo-600/20 text-indigo-400 rounded-lg border border-indigo-500/30">
        <HardDrive class="w-6 h-6" />
      </div>
      <div>
        <h1 class="text-lg font-semibold tracking-tight text-white flex items-center gap-2">
          r2drive
          <span class="text-xs px-2 py-0.5 rounded-full bg-indigo-500/20 text-indigo-300 font-medium border border-indigo-500/30">
            WebConsole
          </span>
        </h1>
        <p class="text-xs text-slate-400">Cloudflare R2 Object Storage Manager</p>
      </div>
    </div>

    <div class="flex items-center space-x-3">
      <span class="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
        <span class="w-2 h-2 rounded-full bg-emerald-500 animate-pulse"></span>
        Connected: {profile}
      </span>
    </div>
  </header>

  <!-- Main Content Body -->
  <main class="flex-1 max-w-7xl w-full mx-auto p-6 space-y-6">
    <!-- Breadcrumbs / Action Toolbar -->
    <div class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 bg-slate-950/40 p-4 rounded-xl border border-slate-800/80">
      <div class="flex items-center space-x-2 text-sm text-slate-300">
        <Folder class="w-4 h-4 text-indigo-400" />
        <span class="font-mono text-xs bg-slate-800 px-2 py-1 rounded text-slate-200">
          r2://{profile}/{prefix || ''}
        </span>
      </div>

      <div class="flex items-center space-x-2">
        <button
          type="button"
          class="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 transition"
        >
          <RefreshCw class="w-3.5 h-3.5" />
          <span>Refresh</span>
        </button>
        <button
          type="button"
          class="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-indigo-600 hover:bg-indigo-500 text-white shadow-sm transition"
        >
          <Upload class="w-3.5 h-3.5" />
          <span>Upload</span>
        </button>
      </div>
    </div>

    <!-- Empty State Preview -->
    <div class="border border-dashed border-slate-800 rounded-2xl p-12 text-center flex flex-col items-center justify-center space-y-4 bg-slate-950/20">
      <div class="p-4 rounded-full bg-slate-800/50 text-slate-400">
        <Cloud class="w-10 h-10 stroke-[1.5]" />
      </div>
      <div class="space-y-1">
        <h3 class="text-base font-medium text-slate-200">No objects displayed yet</h3>
        <p class="text-xs text-slate-400 max-w-md">
          WebConsole setup complete. In subsequent tasks, live bucket browsing and direct-to-R2 resumable multipart uploads will be attached here.
        </p>
      </div>
    </div>
  </main>
</div>
