<script lang="ts">
  import { periodLabel, periods, span } from './budget'
  import { data, ui } from './store.svelte'

  const all = $derived(periods(data.txns, data.plans))
  const p = $derived(all[Math.min(ui.period, all.length - 1)])
  const s = $derived(p ? span(p) : null)
</script>

{#if p}
  <div class="period">
    <button class="ghost" onclick={() => (ui.period = Math.min(all.length - 1, ui.period + 1))} disabled={ui.period >= all.length - 1} aria-label="Earlier period">‹</button>
    <div class="label">
      <h2>{periodLabel(p)}</h2>
      {#if p.end === null && s && p.kind === 'cycle'}<span class="muted small">day {s.elapsed} of ~{s.total}</span>{/if}
    </div>
    <button class="ghost" onclick={() => (ui.period = Math.max(0, ui.period - 1))} disabled={ui.period === 0} aria-label="Later period">›</button>
  </div>
{/if}

<style>
  .period {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.25rem;
  }
  .label {
    min-width: 12rem;
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  h2 {
    font-size: 1.1rem;
  }
  button {
    font-size: 1.3rem;
    line-height: 1;
    padding: 0.2rem 0.6rem;
  }
</style>
