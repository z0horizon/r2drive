<script lang="ts">
  import { bucketStore } from '$lib/stores/bucket.svelte';
  import {
    AlertTriangle,
    Check,
    Copy,
    ExternalLink,
    RefreshCw,
    ShieldAlert,
    X,
  } from 'lucide-svelte';

  let {
    open = false,
    onclose,
    onClose,
  }: {
    open: boolean;
    onclose?: () => void;
    onClose?: () => void;
  } = $props();

  let copied = $state(false);
  let copyTimeout: ReturnType<typeof setTimeout> | null = null;
  let isChecking = $state(false);
  let checkResult = $state<'idle' | 'success' | 'failed'>('idle');

  const currentOrigin =
    typeof window !== 'undefined' && window.location.origin
      ? window.location.origin
      : 'http://localhost:8080';

  const corsSnippet = [
    {
      AllowedOrigins: [currentOrigin, '*'],
      AllowedMethods: ['GET', 'PUT', 'POST', 'DELETE', 'HEAD'],
      AllowedHeaders: ['*'],
      ExposeHeaders: ['ETag'],
      MaxAgeSeconds: 3600,
    },
  ];

  const corsJson = JSON.stringify(corsSnippet, null, 2);

  function handleClose() {
    checkResult = 'idle';
    onclose?.();
    onClose?.();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') {
      handleClose();
    }
  }

  async function handleCopy() {
    try {
      if (typeof navigator !== 'undefined' && navigator.clipboard) {
        await navigator.clipboard.writeText(corsJson);
      }
      copied = true;
      if (copyTimeout) clearTimeout(copyTimeout);
      copyTimeout = setTimeout(() => {
        copied = false;
      }, 2000);
    } catch {
      // Clipboard write fallback
    }
  }

  async function handleRecheck() {
    isChecking = true;
    checkResult = 'idle';
    try {
      await bucketStore.checkCors(bucketStore.selectedProfile, true);
      if (bucketStore.corsStatus === 'healthy') {
        checkResult = 'success';
        setTimeout(() => {
          handleClose();
        }, 600);
      } else {
        checkResult = 'failed';
      }
    } catch {
      checkResult = 'failed';
    } finally {
      isChecking = false;
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm"
    role="dialog"
    aria-modal="true"
    aria-labelledby="cors-dialog-title"
  >
    <div
      class="w-full max-w-xl rounded-2xl border border-slate-800 bg-slate-900 p-6 shadow-2xl space-y-5 text-slate-100 max-h-[90vh] overflow-y-auto"
    >
      <!-- Header -->
      <div class="flex items-start justify-between gap-3">
        <div class="flex items-center gap-3">
          <div
            class="flex items-center justify-center w-10 h-10 rounded-xl bg-amber-500/10 border border-amber-500/20 text-amber-400 shrink-0"
          >
            <ShieldAlert class="w-5 h-5" />
          </div>
          <div>
            <h3 id="cors-dialog-title" class="text-base font-semibold text-white">
              Cấu hình CORS cho Bucket
            </h3>
            <p class="text-xs text-slate-400">
              Cloudflare R2 yêu cầu quy tắc CORS để trình duyệt có thể upload trực tiếp
            </p>
          </div>
        </div>
        <button
          type="button"
          onclick={handleClose}
          class="p-1 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition"
          aria-label="Đóng"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Cloudflare Deep Link Banner -->
      <div
        class="flex items-center justify-between gap-3 p-3 rounded-xl bg-slate-950/60 border border-slate-800 text-xs text-slate-300"
      >
        <span class="truncate">Truy cập trang quản lý bucket trên Cloudflare Dashboard:</span>
        <a
          href="https://dash.cloudflare.com/?to=/:account/r2/overview"
          target="_blank"
          rel="noopener noreferrer"
          class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white font-medium shrink-0 transition"
        >
          <span>Cloudflare Dashboard</span>
          <ExternalLink class="w-3.5 h-3.5" />
        </a>
      </div>

      <!-- 3-Step Guide -->
      <div class="space-y-2">
        <h4 class="text-xs font-semibold uppercase tracking-wider text-slate-400">
          Hướng dẫn 3 bước cấu hình
        </h4>
        <ol class="space-y-2 text-xs text-slate-300 list-decimal list-inside bg-slate-950/40 p-3.5 rounded-xl border border-slate-800">
          <li class="leading-relaxed">
            Mở <strong>Cloudflare R2 Dashboard</strong>, chọn bucket <span class="font-mono text-indigo-400">{bucketStore.selectedProfile || 'hiện tại'}</span> và chuyển sang tab <strong>Settings</strong>.
          </li>
          <li class="leading-relaxed">
            Cuộn xuống phần <strong>CORS Policy</strong>, chọn <strong>Add CORS policy</strong> (hoặc <strong>Edit</strong> nếu đã có).
          </li>
          <li class="leading-relaxed">
            Sao chép đoạn mã JSON bên dưới, dán vào ô cấu hình và nhấn <strong>Save</strong>.
          </li>
        </ol>
      </div>

      <!-- JSON Snippet Code Block with 1-Click Copy -->
      <div class="space-y-2">
        <div class="flex items-center justify-between">
          <span class="text-xs font-semibold uppercase tracking-wider text-slate-400">
            CORS JSON Policy
          </span>
          <button
            type="button"
            onclick={handleCopy}
            class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 transition"
          >
            {#if copied}
              <Check class="w-3.5 h-3.5 text-emerald-400" />
              <span class="text-emerald-400 font-medium">Đã copy!</span>
            {:else}
              <Copy class="w-3.5 h-3.5" />
              <span>Copy JSON</span>
            {/if}
          </button>
        </div>

        <div class="relative">
          <pre
            class="bg-slate-950 p-3.5 rounded-xl border border-slate-800 text-xs font-mono text-emerald-400 overflow-x-auto select-all leading-relaxed"
          >{corsJson}</pre>
        </div>
      </div>

      <!-- Feedback status if re-check failed / success -->
      {#if checkResult === 'failed'}
        <div
          class="flex items-start gap-2.5 rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-xs text-amber-300"
        >
          <AlertTriangle class="w-4 h-4 shrink-0 text-amber-400 mt-0.5" />
          <span>CORS vẫn chưa được kích hoạt. Hãy đảm bảo bạn đã nhấn <strong>Save</strong> trên Cloudflare Dashboard và thử lại sau vài giây (Cloudflare có thể mất 5-10 giây để cập nhật).</span>
        </div>
      {:else if checkResult === 'success'}
        <div
          class="flex items-start gap-2.5 rounded-lg border border-emerald-500/30 bg-emerald-500/10 p-3 text-xs text-emerald-300"
        >
          <Check class="w-4 h-4 shrink-0 text-emerald-400 mt-0.5" />
          <span>Kiểm tra thành công! CORS đã được cấu hình hợp lệ. Đang đóng cửa sổ...</span>
        </div>
      {/if}

      <!-- Actions -->
      <div class="flex items-center justify-end gap-3 pt-2">
        <button
          type="button"
          onclick={handleClose}
          class="px-4 py-2 rounded-lg text-xs font-medium text-slate-300 hover:bg-slate-800 hover:text-white border border-slate-700 transition"
        >
          Đóng
        </button>
        <button
          type="button"
          onclick={handleRecheck}
          disabled={isChecking || bucketStore.corsStatus === 'checking'}
          class="inline-flex items-center gap-1.5 px-4 py-2 rounded-lg text-xs font-medium text-white bg-indigo-600 hover:bg-indigo-500 transition shadow-sm disabled:opacity-50"
        >
          {#if isChecking || bucketStore.corsStatus === 'checking'}
            <RefreshCw class="w-3.5 h-3.5 animate-spin" />
            <span>Đang kiểm tra...</span>
          {:else}
            <RefreshCw class="w-3.5 h-3.5" />
            <span>Kiểm tra lại (Re-check)</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}
