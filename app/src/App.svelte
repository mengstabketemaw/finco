<script lang="ts">
  import Activity from './lib/Activity.svelte'
  import Home from './lib/Home.svelte'
  import Plans from './lib/Plans.svelte'
  import Review from './lib/Review.svelte'
  import SettingsView from './lib/Settings.svelte'
  import TxnSheet from './lib/TxnSheet.svelte'
  import Icon from './lib/Icon.svelte'
  import { listen } from '@tauri-apps/api/event'
  import { inApp } from './lib/api'
  import { data, reload, ui, type Tab } from './lib/store.svelte'

  const tabs: [Tab, string][] = [
    ['home', 'Home'],
    ['review', 'Review'],
    ['activity', 'Activity'],
    ['plans', 'Plans'],
    ['settings', 'Settings'],
  ]

  // The open tab lives in the URL hash, so a reload keeps it.
  const fromHash = location.hash.slice(1) as Tab
  if (tabs.some(([id]) => id === fromHash)) ui.tab = fromHash
  $effect(() => {
    history.replaceState(null, '', `#${ui.tab}`)
  })

  const toReview = $derived(data.txns.filter((t) => t.flow === 'spend' && !t.category && t.counted > 0).length)

  $effect(() => {
    reload()
    const onFocus = () => reload()
    window.addEventListener('focus', onFocus)
    // A new SMS, a notification button or a sync pull changed the data.
    const unlisten = inApp ? listen('data-changed', () => reload()) : null
    return () => {
      window.removeEventListener('focus', onFocus)
      unlisten?.then((f) => f())
    }
  })
</script>

<nav>
  <span class="brand">finco</span>
  {#each tabs as [id, label]}
    <button class="tab" class:active={ui.tab === id} onclick={() => (ui.tab = id)}>
      <span class="icon"><Icon name={id} /></span>
      <span>{label}</span>
      {#if id === 'review' && toReview}<span class="count">{toReview}</span>{/if}
    </button>
  {/each}
</nav>

<main>
  {#if ui.tab === 'home'}
    <Home />
  {:else if ui.tab === 'review'}
    <Review />
  {:else if ui.tab === 'activity'}
    <Activity />
  {:else if ui.tab === 'plans'}
    <Plans />
  {:else}
    <SettingsView />
  {/if}
</main>

<TxnSheet />

{#if ui.toast}<div class="toast" role="status">{ui.toast}</div>{/if}

<style>
  nav {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    gap: 0.25rem;
    align-items: center;
    padding: 0.5rem 1rem;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }
  .brand {
    font-weight: 700;
    margin-right: 0.75rem;
    color: var(--accent);
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    border-color: transparent;
    background: transparent;
  }
  .tab.active {
    background: var(--surface-2);
    font-weight: 600;
  }
  .icon {
    display: none;
  }
  .count {
    min-width: 1.3rem;
    padding: 0 0.35rem;
    border-radius: 999px;
    background: var(--warn-bg);
    color: var(--warn);
    font-size: 0.75rem;
    font-weight: 700;
    line-height: 1.3rem;
  }
  main {
    max-width: 1100px;
    margin: 0 auto;
    padding: 1rem;
  }
  .toast {
    position: fixed;
    left: 50%;
    bottom: 1.25rem;
    transform: translateX(-50%);
    z-index: 20;
    background: var(--text);
    color: var(--bg);
    padding: 0.55rem 1rem;
    border-radius: 999px;
    font-size: 0.9rem;
  }
  /* Phones: tabs move to a bar at the bottom, in thumb reach. */
  @media (max-width: 560px) {
    nav {
      position: fixed;
      top: auto;
      bottom: 0;
      left: 0;
      right: 0;
      padding: 0.25rem 0.25rem calc(0.25rem + env(safe-area-inset-bottom));
      border-bottom: none;
      border-top: 1px solid var(--border);
      gap: 0;
    }
    .brand {
      display: none;
    }
    .tab {
      flex: 1;
      flex-direction: column;
      gap: 0;
      position: relative;
      padding: 0.3rem 0;
      font-size: 0.72rem;
    }
    .tab.active {
      background: transparent;
      color: var(--accent);
    }
    .icon {
      display: block;
      height: 22px;
    }
    .count {
      position: absolute;
      top: 0;
      left: calc(50% + 0.4rem);
    }
    main {
      padding: 0.75rem 0.75rem 5rem;
    }
    .toast {
      bottom: 5rem;
    }
  }
</style>
