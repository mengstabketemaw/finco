<script lang="ts">
  import type { Txn } from './api'
  import { birr, emojiFor, subtitle, title } from './money'
  import { data, ui } from './store.svelte'

  let { t }: { t: Txn } = $props()

  const sign = $derived(t.flow === 'income' ? '+' : t.flow === 'spend' ? '−' : '')
  const untagged = $derived(t.flow === 'spend' && !t.category && t.counted > 0)
  /** Money that comes back or goes out for someone else: shown, not counted. */
  const parked = $derived(t.settles.length > 0 || t.category === 'lent' || t.category === 'borrowed')
  const share = $derived((t.flow === 'spend' || t.flow === 'income') && !parked && t.repaid > 0)
</script>

<button class="row-btn" onclick={() => (ui.open = t.id)}>
  <span class="badge" class:untagged aria-hidden="true">{emojiFor(t)}</span>
  <span class="text">
    <span class="title">{title(t)}</span>
    <span class="sub muted">{subtitle(t, data.settings.own_numbers)}</span>
  </span>
  <span class="amount" class:in={t.flow === 'income' && !parked} class:quiet={t.flow === 'internal' || t.flow === 'loan' || parked}>
    {#if share}
      {sign}{birr(t.counted)}
      <span class="fee muted">your share of {birr(t.amount)}</span>
    {:else}
      {sign}{birr(t.amount)}
      {#if t.settles.length}<span class="fee muted">↩ pays back</span>
      {:else if t.category === 'lent'}<span class="fee muted">{t.repaid >= t.amount ? 'repaid' : `${birr(t.amount - t.repaid)} owed`}</span>
      {:else if t.category === 'borrowed'}<span class="fee muted">{t.repaid >= t.amount ? 'paid back' : 'you owe'}</span>
      {:else if t.fee}<span class="fee muted">+{birr(t.fee)} fee</span>{/if}
    {/if}
  </span>
</button>

<style>
  .row-btn {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    width: 100%;
    text-align: left;
    border: none;
    border-radius: 0;
    background: transparent;
    padding: 0.55rem 0.25rem;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .title,
  .sub {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sub {
    font-size: 0.8rem;
  }
  .amount {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font-weight: 600;
    white-space: nowrap;
  }
  .fee {
    font-size: 0.72rem;
    font-weight: 400;
  }
  .quiet {
    color: var(--muted);
    font-weight: 400;
  }
  .untagged {
    background: var(--warn-bg);
    color: var(--warn);
    font-weight: 700;
  }
</style>
