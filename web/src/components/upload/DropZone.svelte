<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { bucketStore } from '$lib/stores/bucket.svelte';
  import { formatBytes } from '$lib/utils/format';
  import {
    listSessions,
    deleteSession,
    type UploadManifest,
  } from '$lib/upload/indexeddb';
  import {
    uploadFile,
    resumeInterruptedUpload,
    cleanObjectKey,
  } from '$lib/upload/worker';
  import { abortUpload } from '$lib/api/transfers';
  import {
    UploadCloud,
    RotateCcw,
    Trash2,
    AlertTriangle,
    X,
    FolderUp,
  } from 'lucide-svelte';

  let fileInput: HTMLInputElement | null = $state(null);
  let isDragging = $state(false);
  let dragCounter = 0;
  let pendingSessions = $state<UploadManifest[]>([]);
  let activeResumeTarget = $state<UploadManifest | null>(null);

  // Pending resume prompt when a dropped or selected file matches an existing session
  interface ResumePrompt {
    file: File;
    session: UploadManifest;
  }
  let activeResumePrompts = $state<ResumePrompt[]>([]);

  /**
   * Opens the native OS file picker.
   */
  export function openPicker(): void {
    if (fileInput) {
      fileInput.value = '';
      fileInput.click();
    }
  }

  function selectFileToResume(session: UploadManifest): void {
    activeResumeTarget = session;
    openPicker();
  }

  async function loadPendingSessions(): Promise<void> {
    try {
      const sessions = await listSessions();
      pendingSessions = sessions;
    } catch (err) {
      console.warn('Failed to read IndexedDB sessions:', err);
    }
  }

  onMount(() => {
    loadPendingSessions();

    if (typeof window !== 'undefined') {
      window.addEventListener('dragenter', handleDragEnter);
      window.addEventListener('dragover', handleDragOver);
      window.addEventListener('dragleave', handleDragLeave);
      window.addEventListener('drop', handleDrop);
    }
  });

  onDestroy(() => {
    if (typeof window !== 'undefined') {
      window.removeEventListener('dragenter', handleDragEnter);
      window.removeEventListener('dragover', handleDragOver);
      window.removeEventListener('dragleave', handleDragLeave);
      window.removeEventListener('drop', handleDrop);
    }
  });

  function handleDragEnter(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer?.types?.includes('Files')) {
      dragCounter++;
      isDragging = true;
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = 'copy';
    }
  }

  function handleDragLeave(e: DragEvent) {
    e.preventDefault();
    dragCounter--;
    if (dragCounter <= 0) {
      dragCounter = 0;
      isDragging = false;
    }
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    dragCounter = 0;
    isDragging = false;

    if (e.dataTransfer?.files && e.dataTransfer.files.length > 0) {
      handleFiles(Array.from(e.dataTransfer.files));
    }
  }

  function handleFileInputChange(e: Event) {
    const input = e.target as HTMLInputElement;
    if (input.files && input.files.length > 0) {
      handleFiles(Array.from(input.files));
      input.value = '';
    }
  }

  async function handleFiles(files: File[]): Promise<void> {
    const profile = bucketStore.selectedProfile;
    const prefix = bucketStore.currentPrefix;

    if (!profile) {
      console.warn('Cannot upload: No bucket profile selected');
      return;
    }

    // Refresh pending sessions to ensure up-to-date manifest list
    await loadPendingSessions();

    const filesToProcess = [...files];

    // If user clicked "Select file to resume" for a specific unfinished session in the banner
    if (activeResumeTarget) {
      const target = activeResumeTarget;
      activeResumeTarget = null;

      const expectedName = target.key.split('/').pop() || target.key;
      const matchedIdx = filesToProcess.findIndex(
        (f) => (f.name === expectedName || f.name === target.key) && f.size === target.fileSize
      );

      if (matchedIdx >= 0) {
        const matchedFile = filesToProcess[matchedIdx];
        filesToProcess.splice(matchedIdx, 1);
        activeResumePrompts = [
          ...activeResumePrompts.filter((p) => p.session.uploadId !== target.uploadId),
          { file: matchedFile, session: target },
        ];
      }
    }

    for (const file of filesToProcess) {
      const targetKey = cleanObjectKey(prefix, file.name);
      const matchingSession = pendingSessions.find(
        (s) =>
          s.profile === profile &&
          (s.key === targetKey || s.key.split('/').pop() === file.name) &&
          s.fileSize === file.size
      );

      if (matchingSession) {
        // Prompt user to resume or start fresh
        activeResumePrompts = [
          ...activeResumePrompts.filter((p) => p.session.uploadId !== matchingSession.uploadId),
          { file, session: matchingSession },
        ];
      } else {
        // Direct new upload
        uploadFile(file, profile, prefix).catch((err) => {
          console.error(`Upload error for ${file.name}:`, err);
        });
      }
    }
  }

  async function confirmResume(prompt: ResumePrompt): Promise<void> {
    activeResumePrompts = activeResumePrompts.filter(
      (p) => p.session.uploadId !== prompt.session.uploadId
    );
    try {
      await resumeInterruptedUpload(prompt.file, prompt.session);
    } catch (err) {
      console.error(`Resume error for ${prompt.file.name}:`, err);
    } finally {
      await loadPendingSessions();
    }
  }

  async function discardAndRestart(prompt: ResumePrompt): Promise<void> {
    activeResumePrompts = activeResumePrompts.filter(
      (p) => p.session.uploadId !== prompt.session.uploadId
    );
    try {
      await abortUpload(prompt.session.profile, prompt.session.uploadId);
    } catch (e) {
      console.warn('Failed to abort session on server:', e);
    }
    await deleteSession(prompt.session.uploadId);
    await loadPendingSessions();

    // Start fresh upload
    uploadFile(prompt.file, prompt.session.profile, bucketStore.currentPrefix).catch((err) => {
      console.error(`Fresh upload error for ${prompt.file.name}:`, err);
    });
  }

  function dismissPrompt(prompt: ResumePrompt): void {
    activeResumePrompts = activeResumePrompts.filter(
      (p) => p.session.uploadId !== prompt.session.uploadId
    );
  }

  async function discardSession(session: UploadManifest): Promise<void> {
    try {
      await abortUpload(session.profile, session.uploadId);
    } catch (e) {
      console.warn('Failed to abort session on server:', e);
    }
    await deleteSession(session.uploadId);
    await loadPendingSessions();
  }
</script>

<!-- Hidden File Input for Native File Dialog -->
<input
  bind:this={fileInput}
  type="file"
  multiple
  class="hidden"
  onchange={handleFileInputChange}
/>

<!-- Full-Screen Drag & Drop Overlay -->
{#if isDragging}
  <div
    class="fixed inset-0 z-50 bg-slate-950/85 backdrop-blur-sm flex flex-col items-center justify-center p-6 border-4 border-dashed border-indigo-500 transition-all pointer-events-none"
  >
    <div class="p-6 rounded-full bg-indigo-600/20 text-indigo-400 border border-indigo-500/30 mb-4 animate-bounce">
      <UploadCloud class="w-16 h-16 stroke-[1.5]" />
    </div>
    <h2 class="text-xl font-bold text-white mb-2">Drop files to upload</h2>
    <p class="text-sm text-slate-300 font-mono">
      Destination: {bucketStore.selectedProfile}/{bucketStore.currentPrefix || ''}
    </p>
    <p class="text-xs text-indigo-300/80 mt-2">
      Multipart chunking with 4x concurrency will be applied for files &ge; 10MB
    </p>
  </div>
{/if}

<!-- Resume Confirmation Modal (When dropped file matches an interrupted session) -->
{#if activeResumePrompts.length > 0}
  {@const prompt = activeResumePrompts[0]}
  <div class="fixed inset-0 z-50 bg-slate-950/70 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-slate-900 border border-slate-700 rounded-2xl p-5 max-w-md w-full shadow-2xl space-y-4">
      <div class="flex items-start justify-between">
        <div class="flex items-center space-x-3">
          <div class="p-2.5 bg-amber-500/20 text-amber-400 rounded-xl border border-amber-500/30">
            <RotateCcw class="w-6 h-6" />
          </div>
          <div>
            <h3 class="text-sm font-semibold text-white">Resume Interrupted Upload?</h3>
            <p class="text-xs text-slate-400">Previous upload session found in browser storage</p>
          </div>
        </div>
        <button
          type="button"
          onclick={() => dismissPrompt(prompt)}
          class="text-slate-400 hover:text-slate-200 p-1 rounded-md"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <div class="p-3 rounded-xl bg-slate-950/60 border border-slate-800 space-y-1.5 text-xs text-slate-300 font-mono">
        <div class="flex justify-between">
          <span class="text-slate-500">File:</span>
          <span class="text-slate-200 truncate max-w-[200px]">{prompt.file.name}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-slate-500">Size:</span>
          <span>{formatBytes(prompt.session.fileSize)}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-slate-500">Parts Uploaded:</span>
          <span class="text-emerald-400">{prompt.session.completedParts.length} parts</span>
        </div>
        <div class="flex justify-between">
          <span class="text-slate-500">Target Key:</span>
          <span class="truncate max-w-[200px]">{prompt.session.key}</span>
        </div>
      </div>

      <div class="flex items-center justify-end space-x-2 pt-2">
        <button
          type="button"
          onclick={() => discardAndRestart(prompt)}
          class="px-3 py-1.5 rounded-lg text-xs font-medium text-slate-300 hover:text-white bg-slate-800 hover:bg-slate-700 transition"
        >
          Start Fresh
        </button>
        <button
          type="button"
          onclick={() => confirmResume(prompt)}
          class="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-indigo-600 hover:bg-indigo-500 text-white shadow transition"
        >
          <RotateCcw class="w-3.5 h-3.5" />
          <span>Resume Transfer</span>
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Unfinished Sessions Banner on Page Load -->
{#if pendingSessions.length > 0}
  <div class="rounded-xl border border-amber-500/30 bg-amber-500/10 p-3.5 sm:p-4 text-xs text-amber-200">
    <div class="flex items-start justify-between gap-3">
      <div class="flex items-start space-x-3">
        <AlertTriangle class="w-5 h-5 text-amber-400 shrink-0 mt-0.5" />
        <div class="space-y-1">
          <p class="font-semibold text-amber-100">
            {pendingSessions.length} Unfinished Upload Session{pendingSessions.length === 1 ? '' : 's'} Detected
          </p>
          <p class="text-amber-300/80 leading-relaxed">
            Drop or select matching files into the folder to resume interrupted transfers:
          </p>
          <div class="mt-2 space-y-1.5">
            {#each pendingSessions as session (session.uploadId)}
              <div class="flex flex-wrap items-center justify-between gap-2 p-2 rounded-lg bg-slate-900/60 border border-amber-500/20 text-[11px] font-mono">
                <div class="flex items-center space-x-2 min-w-0">
                  <FolderUp class="w-3.5 h-3.5 text-amber-400 shrink-0" />
                  <span class="text-amber-100 truncate max-w-xs font-medium">{session.key}</span>
                  <span class="text-slate-400">({formatBytes(session.fileSize)})</span>
                  <span class="text-emerald-400">{session.completedParts.length} parts done</span>
                </div>
                <div class="flex items-center space-x-2 shrink-0">
                  <button
                    type="button"
                    onclick={() => selectFileToResume(session)}
                    class="px-2 py-1 rounded bg-indigo-600/30 hover:bg-indigo-600/50 text-indigo-200 border border-indigo-500/40 transition text-[10px] font-sans"
                  >
                    Select file to resume
                  </button>
                  <button
                    type="button"
                    onclick={() => discardSession(session)}
                    title="Discard session and remove from storage"
                    class="p-1 text-slate-400 hover:text-rose-400 hover:bg-rose-500/10 rounded transition"
                  >
                    <Trash2 class="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>
            {/each}
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}
