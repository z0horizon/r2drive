<script lang="ts">
  import { formatBytes } from '$lib/utils/format';
  import type { UploadStatus } from '$lib/stores/upload.svelte';

  let {
    progress = 0,
    uploadedBytes = 0,
    totalBytes = 0,
    speed = 0,
    status = 'uploading',
    error = '',
  }: {
    progress?: number;
    uploadedBytes?: number;
    totalBytes?: number;
    speed?: number;
    status?: UploadStatus;
    error?: string;
  } = $props();

  let barColor = $derived(() => {
    switch (status) {
      case 'completed':
        return 'bg-emerald-500';
      case 'failed':
        return 'bg-rose-500';
      case 'aborted':
        return 'bg-slate-500';
      case 'queued':
        return 'bg-amber-500';
      default:
        return 'bg-indigo-500';
    }
  });

  let statusText = $derived(() => {
    switch (status) {
      case 'completed':
        return 'Completed';
      case 'failed':
        return error || 'Upload failed';
      case 'aborted':
        return 'Cancelled';
      case 'queued':
        return 'Queued';
      default:
        return `${formatBytes(uploadedBytes)} of ${formatBytes(totalBytes)}`;
    }
  });
</script>

<div class="w-full space-y-1.5 font-sans">
  <!-- Progress Header -->
  <div class="flex items-center justify-between text-xs">
    <span
      class={`truncate max-w-[200px] sm:max-w-[240px] font-medium ${
        status === 'failed'
          ? 'text-rose-400'
          : status === 'completed'
            ? 'text-emerald-400'
            : status === 'aborted'
              ? 'text-slate-400'
              : 'text-slate-300'
      }`}
      title={error || ''}
    >
      {statusText()}
    </span>

    <div class="flex items-center space-x-2 text-right shrink-0">
      {#if status === 'uploading' && speed > 0}
        <span class="text-[11px] text-slate-400 font-mono">
          {formatBytes(speed)}/s
        </span>
      {/if}
      <span class="text-xs font-medium font-mono text-slate-300">
        {Math.round(progress)}%
      </span>
    </div>
  </div>

  <!-- Progress Bar Track -->
  <div class="h-1.5 w-full bg-slate-800 rounded-full overflow-hidden">
    <div
      class={`h-full rounded-full transition-all duration-300 ease-out ${barColor()}`}
      style={`width: ${Math.min(100, Math.max(0, progress))}%`}
    ></div>
  </div>
</div>
