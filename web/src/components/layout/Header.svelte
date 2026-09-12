<script lang="ts">
  import { onMount } from 'svelte';
  import { authStore } from '$lib/stores/auth.svelte';
  import { bucketStore } from '$lib/stores/bucket.svelte';
  import { HardDrive, RefreshCw, LogOut, Sun, Moon } from 'lucide-svelte';

  let isDark = $state(true);

  onMount(() => {
    const saved = typeof localStorage !== 'undefined' ? localStorage.getItem('r2drive-theme') : null;
    if (saved === 'light') {
      isDark = false;
      document.documentElement.classList.remove('dark');
    } else {
      isDark = true;
      document.documentElement.classList.add('dark');
    }
  });

  function toggleTheme() {
    isDark = !isDark;
    if (typeof document !== 'undefined') {
      if (isDark) {
        document.documentElement.classList.add('dark');
        localStorage.setItem('r2drive-theme', 'dark');
      } else {
        document.documentElement.classList.remove('dark');
        localStorage.setItem('r2drive-theme', 'light');
      }
    }
  }

  async function handleProfileChange(event: Event) {
    const select = event.target as HTMLSelectElement;
    if (select.value && select.value !== bucketStore.selectedProfile) {
      await bucketStore.setProfile(select.value);
    }
  }

  async function handleRefresh() {
    if (bucketStore.loading) return;
    await bucketStore.refresh();
  }

  async function handleLogout() {
    await authStore.logout();
    bucketStore.reset();
  }
</script>

<header class="border-b border-slate-800 bg-slate-950/80 backdrop-blur px-4 sm:px-6 py-3.5 flex items-center justify-between sticky top-0 z-20">
  <div class="flex items-center space-x-3">
    <div class="p-2 bg-indigo-600/20 text-indigo-400 rounded-lg border border-indigo-500/30">
      <HardDrive class="w-6 h-6" />
    </div>
    <div>
      <h1 class="text-base sm:text-lg font-semibold tracking-tight text-white flex items-center gap-2">
        r2drive
        <span class="text-[11px] px-2 py-0.5 rounded-full bg-indigo-500/20 text-indigo-300 font-medium border border-indigo-500/30">
          WebConsole
        </span>
      </h1>
      <p class="text-xs text-slate-400 hidden sm:block">Cloudflare R2 Object Storage Manager</p>
    </div>
  </div>

  <div class="flex items-center space-x-2 sm:space-x-3">
    <!-- Dark / Light Theme Toggle -->
    <button
      type="button"
      onclick={toggleTheme}
      title={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
      class="inline-flex items-center space-x-1.5 px-2.5 py-1.5 rounded-lg text-xs font-medium bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 transition"
    >
      {#if isDark}
        <Sun class="w-3.5 h-3.5 text-amber-400" />
        <span class="hidden md:inline">Light</span>
      {:else}
        <Moon class="w-3.5 h-3.5 text-indigo-400" />
        <span class="hidden md:inline">Dark</span>
      {/if}
    </button>

    {#if authStore.isAuthenticated}
      <!-- Bucket Profile Selector Dropdown -->
      {#if bucketStore.profiles.length > 0}
        <div class="flex items-center space-x-1.5">
          <label for="profile-selector" class="text-xs text-slate-400 hidden md:inline font-medium">
            Profile:
          </label>
          <div class="relative">
            <select
              id="profile-selector"
              value={bucketStore.selectedProfile}
              onchange={handleProfileChange}
              disabled={bucketStore.loading}
              class="appearance-none bg-slate-900 text-slate-200 text-xs rounded-lg pl-2.5 pr-8 py-1.5 border border-slate-700 hover:border-slate-600 focus:outline-none focus:border-indigo-500 font-medium transition cursor-pointer disabled:opacity-50"
            >
              {#each bucketStore.profiles as profile}
                <option value={profile.name}>
                  {profile.name} {profile.default ? '(default)' : ''}
                </option>
              {/each}
            </select>
            <div class="pointer-events-none absolute inset-y-0 right-0 flex items-center px-2 text-slate-400">
              <svg class="w-3 h-3 fill-current" viewBox="0 0 20 20">
                <path d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" />
              </svg>
            </div>
          </div>
        </div>
      {/if}

      <!-- Connected Profile Indicator -->
      <span
        class="hidden lg:inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20"
        title={bucketStore.selectedProfile ? `Connected to ${bucketStore.selectedProfile}` : 'Connected'}
      >
        <span class="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse"></span>
        <span>Connected: {bucketStore.selectedProfile || 'Default'}</span>
      </span>

      <!-- Force Refresh Button -->
      <button
        type="button"
        onclick={handleRefresh}
        disabled={bucketStore.loading}
        title="Sync directly from Cloudflare R2"
        class="inline-flex items-center space-x-1.5 px-2.5 py-1.5 rounded-lg text-xs font-medium bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 transition disabled:opacity-50"
      >
        <RefreshCw class={`w-3.5 h-3.5 ${bucketStore.loading ? 'animate-spin text-indigo-400' : ''}`} />
        <span class="hidden sm:inline">Refresh</span>
      </button>

      <!-- Logout Button -->
      <button
        type="button"
        onclick={handleLogout}
        title="Sign out of WebConsole"
        class="inline-flex items-center space-x-1.5 px-2.5 py-1.5 rounded-lg text-xs font-medium bg-slate-800 hover:bg-rose-950/40 text-slate-300 hover:text-rose-300 border border-slate-700 hover:border-rose-800/60 transition"
      >
        <LogOut class="w-3.5 h-3.5" />
        <span class="hidden sm:inline">Logout</span>
      </button>
    {/if}
  </div>
</header>
