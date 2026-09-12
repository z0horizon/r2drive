<script lang="ts">
  import { authStore } from '$lib/stores/auth.svelte';
  import { bucketStore } from '$lib/stores/bucket.svelte';
  import { HardDrive, Lock, RefreshCw, AlertCircle } from 'lucide-svelte';

  let { onSuccess }: { onSuccess?: () => void } = $props();

  let password = $state('');
  let isSubmitting = $state(false);
  let errorMessage = $state<string | null>(null);

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    if (!password.trim() || isSubmitting) return;

    isSubmitting = true;
    errorMessage = null;

    try {
      await authStore.login(password.trim());
      password = '';
      await bucketStore.loadProfiles();
      onSuccess?.();
    } catch (err) {
      errorMessage = err instanceof Error ? err.message : 'Authentication failed';
    } finally {
      isSubmitting = false;
    }
  }
</script>

<div
  class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm"
  role="dialog"
  aria-modal="true"
  aria-labelledby="login-dialog-title"
>
  <div class="w-full max-w-md rounded-2xl border border-slate-800 bg-slate-900 p-6 shadow-2xl space-y-6">
    <div class="text-center space-y-3">
      <div class="inline-flex items-center justify-center w-12 h-12 rounded-xl bg-indigo-500/10 border border-indigo-500/20 text-indigo-400 mx-auto">
        <HardDrive class="w-6 h-6" />
      </div>
      <div>
        <h2 id="login-dialog-title" class="text-lg font-semibold tracking-tight text-white flex items-center justify-center gap-2">
          r2drive
          <span class="text-xs px-2 py-0.5 rounded-full bg-indigo-500/20 text-indigo-300 font-medium border border-indigo-500/30">
            WebConsole
          </span>
        </h2>
        <p class="text-xs text-slate-400 mt-1">Enter your admin password to access bucket storage</p>
      </div>
    </div>

    <form onsubmit={handleSubmit} class="space-y-4">
      <div class="space-y-1.5 text-left">
        <label for="admin-password" class="block text-xs font-medium text-slate-300">
          Admin Password
        </label>
        <input
          id="admin-password"
          type="password"
          bind:value={password}
          placeholder="••••••••••••"
          autocomplete="current-password"
          disabled={isSubmitting}
          class="w-full rounded-lg border border-slate-700 bg-slate-800/80 px-3.5 py-2.5 text-sm text-slate-100 placeholder-slate-500 transition focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500 disabled:opacity-50"
        />
      </div>

      {#if errorMessage || authStore.error}
        <div class="flex items-start gap-2.5 rounded-lg border border-rose-500/30 bg-rose-500/10 p-3 text-xs text-rose-300 text-left">
          <AlertCircle class="w-4 h-4 shrink-0 text-rose-400 mt-0.5" />
          <span>{errorMessage || authStore.error}</span>
        </div>
      {/if}

      <button
        type="submit"
        disabled={isSubmitting || !password.trim()}
        class="w-full flex items-center justify-center gap-2 rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-medium text-white transition hover:bg-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:ring-offset-slate-900 disabled:cursor-not-allowed disabled:opacity-50 shadow-sm"
      >
        {#if isSubmitting}
          <RefreshCw class="w-4 h-4 animate-spin" />
          <span>Authenticating...</span>
        {:else}
          <Lock class="w-4 h-4" />
          <span>Unlock WebConsole</span>
        {/if}
      </button>
    </form>
  </div>
</div>
