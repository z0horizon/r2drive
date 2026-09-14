<script lang="ts">
  import { bucketStore } from '$lib/stores/bucket.svelte';
  import { AlertTriangle, RefreshCw } from 'lucide-svelte';

  let {
    onConfigure,
    onOpenModal,
  }: {
    onConfigure?: () => void;
    onOpenModal?: () => void;
  } = $props();

  let isChecking = $state(false);

  function handleOpen() {
    onConfigure?.();
    onOpenModal?.();
  }

  async function handleRecheck() {
    if (isChecking || bucketStore.corsStatus === 'checking') return;
    isChecking = true;
    try {
      await bucketStore.checkCors(bucketStore.selectedProfile, true);
    } finally {
      isChecking = false;
    }
  }
</script>

{#if bucketStore.corsStatus === 'blocked' || isChecking}
  <div
    class="bg-amber-50 dark:bg-amber-950/30 border-b border-amber-200 dark:border-amber-800 px-4 sm:px-6 py-2.5 sm:py-3 transition-colors duration-150"
    role="alert"
  >
    <div class="max-w-7xl mx-auto flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
      <div class="flex items-center gap-2.5 text-xs sm:text-sm text-amber-900 dark:text-amber-200">
        <AlertTriangle class="w-4 h-4 sm:w-5 sm:h-5 shrink-0 text-amber-600 dark:text-amber-400" />
        <span class="font-medium">
          Bucket chưa được cấu hình CORS. Trình duyệt không thể upload trực tiếp lên Cloudflare R2.
        </span>
      </div>

      <div class="flex items-center space-x-2 shrink-0 self-end sm:self-auto">
        <button
          type="button"
          onclick={handleOpen}
          class="inline-flex items-center px-3 py-1.5 rounded-lg text-xs font-medium bg-amber-600 dark:bg-amber-500 hover:bg-amber-700 dark:hover:bg-amber-400 text-white dark:text-slate-950 transition shadow-sm cursor-pointer"
        >
          Xem hướng dẫn cấu hình
        </button>
        <button
          type="button"
          onclick={handleRecheck}
          disabled={isChecking || bucketStore.corsStatus === 'checking'}
          class="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-amber-100 dark:bg-amber-900/40 hover:bg-amber-200 dark:hover:bg-amber-800/60 text-amber-900 dark:text-amber-200 border border-amber-300 dark:border-amber-700/60 transition disabled:opacity-50 cursor-pointer"
        >
          <RefreshCw
            class={`w-3.5 h-3.5 ${isChecking || bucketStore.corsStatus === 'checking' ? 'animate-spin text-amber-600 dark:text-amber-400' : ''}`}
          />
          <span>Kiểm tra lại</span>
        </button>
      </div>
    </div>
  </div>
{/if}
