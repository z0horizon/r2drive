<script lang="ts">
  import { FolderOpen } from 'lucide-svelte';

  let {
    title = 'This folder is empty',
    message = 'No objects or subdirectories found under this prefix.',
  }: {
    title?: string;
    message?: string;
  } = $props();
</script>

<div class="border border-dashed border-slate-800 rounded-2xl p-12 text-center flex flex-col items-center justify-center space-y-3 bg-slate-950/20 my-4">
  <div class="p-3.5 rounded-full bg-slate-800/60 text-slate-400 border border-slate-700/40">
    <FolderOpen class="w-8 h-8 stroke-[1.5] text-slate-400" />
  </div>
  <div class="space-y-1">
    <h3 class="text-sm font-medium text-slate-200">{title}</h3>
    <p class="text-xs text-slate-400 max-w-sm">{message}</p>
  </div>
</div>
