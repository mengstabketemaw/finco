<script lang="ts">
  // The "paying back" part of a transaction's sheet:
  // - money in (or a payment repaying a loan): pick what it pays back
  // - a repayment already linked: what it pays back, with Unlink
  // - lent money: how much is still owed, with Write off
  // - a split bill: how much came back
  import { api, type Suggestion, type Txn } from './api'
  import { birr, dayLabel, title } from './money'
  import { categorize, data, reload, toast, ui } from './store.svelte'

  let { t }: { t: Txn } = $props()

  let suggestions = $state<Suggestion[]>([])
  let picked = $state<string[]>([])
  let showAll = $state(false)

  const linked = $derived(t.settles.map((id) => data.txns.find((x) => x.id === id)).filter((x): x is Txn => !!x))
  const repaidBy = $derived(data.txns.filter((x) => x.settles.includes(t.id)))
  /** Money in can pay back a payment; money out can pay back money borrowed. */
  const canSettle = $derived(t.settles.length === 0 && (t.flow === 'income' || (t.flow === 'spend' && t.kind === 'transfer_out')))

  $effect(() => {
    const id = t.id
    picked = []
    showAll = false
    suggestions = []
    if (canSettle) api.suggest(id).then((s) => (suggestions = s))
  })

  const shown = $derived(showAll ? suggestions : suggestions.slice(0, 3))
  const pickedTotal = $derived(suggestions.filter((s) => picked.includes(s.txn.id)).reduce((sum, s) => sum + s.remaining, 0))

  async function link(ids: string[]) {
    await api.link(t.id, ids)
    await reload()
    toast(ids.length ? 'Linked: not counted as income or spending' : 'Unlinked')
  }

  function toggle(id: string) {
    picked = picked.includes(id) ? picked.filter((x) => x !== id) : [...picked, id]
  }
</script>

{#if linked.length}
  <section class="box">
    <strong>↩ Pays back</strong>
    {#each linked as p (p.id)}
      <button class="ghost line" onclick={() => (ui.open = p.id)}>
        <span>{title(p)} · {dayLabel(p.occurred_at)}</span><span>{birr(p.amount)}</span>
      </button>
    {/each}
    <p class="small muted">So this money isn't counted as {t.flow === 'income' ? 'income' : 'spending'}.</p>
    <div><button class="small" onclick={() => link([])}>Unlink</button></div>
  </section>
{/if}

{#if t.category === 'lent' || t.category === 'borrowed'}
  {@const open = t.amount - t.repaid}
  <section class="box">
    <strong>{t.category === 'lent' ? '🤲 Lent' : '🫴 Borrowed'}</strong>
    <p class="small">
      {#if open <= 0}Paid back in full.
      {:else if t.category === 'lent'}{birr(open)} still owed to you{t.repaid ? ` (${birr(t.repaid)} back so far)` : ''}. Not counted as spending.
      {:else}You still owe {birr(open)}. Not counted as income.{/if}
    </p>
    {#if t.category === 'lent' && open > 0}
      <div class="row">
        <button class="small" onclick={() => categorize(t, 'written_off', false)}>Write off: count it as spent</button>
      </div>
    {/if}
  </section>
{:else if repaidBy.length && t.flow === 'spend'}
  <section class="box">
    <strong>Split</strong>
    <p class="small">{birr(t.repaid)} came back, so your share is <strong>{birr(t.counted)}</strong>.</p>
    {#each repaidBy as r (r.id)}
      <button class="ghost line" onclick={() => (ui.open = r.id)}>
        <span>{title(r)} · {dayLabel(r.occurred_at)}</span><span>{birr(r.amount)}</span>
      </button>
    {/each}
  </section>
{/if}

{#if canSettle && suggestions.length}
  <section class="box">
    <strong>{t.flow === 'income' ? 'Paying you back?' : 'Paying back money you borrowed?'}</strong>
    <p class="small muted">Pick what it pays back. It then isn't counted, and {t.flow === 'income' ? 'the payment counts only your share' : 'neither is the loan'}.</p>
    {#each shown as s (s.txn.id)}
      <label class="pick">
        <input type="checkbox" checked={picked.includes(s.txn.id)} onchange={() => toggle(s.txn.id)} />
        <span class="what">
          {title(s.txn)}{#if s.txn.category === 'lent'} · lent{/if}
          <span class="muted small">{dayLabel(s.txn.occurred_at)}{s.remaining !== s.txn.amount ? ` · ${birr(s.remaining)} open` : ''}</span>
        </span>
        <span>{birr(s.txn.amount)}</span>
      </label>
    {/each}
    {#if suggestions.length > 3 && !showAll}
      <button class="ghost small more" onclick={() => (showAll = true)}>{suggestions.length - 3} more</button>
    {/if}
    <div class="row">
      <button class="primary small" disabled={!picked.length} onclick={() => link(picked)}>
        {picked.length ? `It pays back ${birr(Math.min(pickedTotal, t.amount))}` : 'Pick one'}
      </button>
    </div>
  </section>
{/if}

<style>
  .box {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    background: var(--surface-2);
    border-radius: 8px;
    padding: 0.6rem 0.75rem;
  }
  p {
    margin: 0;
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.15rem 0;
    text-align: left;
    font-size: 0.9rem;
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.9rem;
  }
  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .more {
    align-self: flex-start;
    padding: 0;
    color: var(--accent);
  }
  .small {
    font-size: 0.82rem;
  }
</style>
