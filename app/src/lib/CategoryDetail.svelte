<script lang="ts">
  // "Where it went", one category: every payment behind its total, with
  // notes, where the money went and how much of it counted here.
  import type { Txn } from './api'
  import { SOURCES, birr, category, dayLabel, day, pieces, time, title } from './money'
  import { ui } from './store.svelte'

  let { id, list, period, onclose }: { id: string; list: Txn[]; period: string; onclose: () => void } = $props()

  const cat = $derived(category(id))
  let sort = $state<'date' | 'amount'>('date')

  /** Each payment with the amount that counts under this category. */
  const rows = $derived.by(() => {
    const out: { t: Txn; amount: number }[] = []
    for (const t of list) {
      // Fees: every charge on a transaction and fee-only transactions, as
      // well as payments tagged Fees.
      let a = id === 'fees' ? t.fee + (t.kind === 'fee' && t.flow === 'spend' ? t.counted : 0) : 0
      if (t.flow === 'spend' && t.kind !== 'fee') {
        a += pieces(t)
          .filter((p) => p.category === id)
          .reduce((s, p) => s + p.amount, 0)
      }
      if (a > 0) out.push({ t, amount: a })
    }
    return out.sort((a, b) => (sort === 'amount' ? b.amount - a.amount : b.t.occurred_at.localeCompare(a.t.occurred_at)))
  })
  const total = $derived(rows.reduce((s, r) => s + r.amount, 0))

  /** By date: grouped under day headings. By amount: one list. */
  const groups = $derived.by(() => {
    if (sort === 'amount') return [{ d: '', rows }]
    const m = new Map<string, typeof rows>()
    for (const r of rows) {
      const d = day(r.t)
      if (!m.has(d)) m.set(d, [])
      m.get(d)!.push(r)
    }
    return [...m].map(([d, rows]) => ({ d, rows }))
  })

  /** Who got the most here. */
  const top = $derived.by(() => {
    const m = new Map<string, number>()
    for (const r of rows) m.set(title(r.t), (m.get(title(r.t)) ?? 0) + r.amount)
    return [...m].sort((a, b) => b[1] - a[1]).slice(0, 3)
  })

  /** Where the money went: from which account, to which account or number. */
  function where(t: Txn): string {
    const from = SOURCES[t.source] ?? t.source
    const to = t.target_number ?? t.counterparty_account ?? (t.description && t.description !== title(t) ? t.description : null)
    return to ? `${from} → ${to}` : from
  }

  /** Why the amount here differs from what was paid. */
  function why(t: Txn, amount: number): string | null {
    if (id === 'fees' && t.fee === amount && t.kind !== 'fee') return `fee on ${birr(t.amount)}`
    if (amount === t.amount) return null
    if (t.splits.some((p) => p.category === id)) return `part of ${birr(t.amount)}`
    if (t.repaid) return `your share of ${birr(t.amount)}`
    return `of ${birr(t.amount)}`
  }
</script>

<!-- Capture phase: runs before the sheet's handler clears ui.open, so Esc closes only the sheet on top. -->
<svelte:window onkeydowncapture={(e) => e.key === 'Escape' && !ui.open && onclose()} />

<div class="backdrop" onclick={onclose} role="presentation"></div>
<div class="modal card" role="dialog" aria-modal="true" aria-label="{cat.label} details">
  <header>
    <div class="row">
      <span class="emoji" aria-hidden="true">{cat.emoji}</span>
      <div>
        <h3>{cat.label}</h3>
        <span class="muted small">{period} · {rows.length} payment{rows.length === 1 ? '' : 's'}</span>
      </div>
      <span class="spacer"></span>
      <strong class="total">ETB {birr(total, true)}</strong>
      <button class="ghost" onclick={onclose} aria-label="Close">✕</button>
    </div>
    {#if top.length > 1 && id !== 'fees'}
      <p class="small muted top">
        Most to {#each top as [name, a], i}{i ? ', ' : ''}<strong>{name}</strong> {birr(a, true)}{/each}
      </p>
    {/if}
    <div class="toggle" role="group" aria-label="Sort">
      <button class:active={sort === 'date'} onclick={() => (sort = 'date')}>Newest</button>
      <button class:active={sort === 'amount'} onclick={() => (sort = 'amount')}>Biggest</button>
    </div>
  </header>

  <div class="body">
    {#each groups as g (g.d)}
      {#if g.d}<div class="day small">{dayLabel(g.d)}</div>{/if}
      {#each g.rows as { t, amount } (t.id)}
        {@const extra = why(t, amount)}
        <button class="item" onclick={() => (ui.open = t.id)}>
          <span class="who">
            <span class="name">{title(t)}</span>
            <span class="sub muted">{#if !g.d}{dayLabel(day(t))} · {/if}{time(t)} · {where(t)}</span>
            {#if t.note}<span class="note">“{t.note}”</span>{/if}
          </span>
          <span class="amt">
            {birr(amount)}
            {#if extra}<span class="sub muted">{extra}</span>{/if}
          </span>
        </button>
      {/each}
    {/each}
  </div>
</div>

<style>
  /* Under the transaction sheet, which opens on top when a row is tapped. */
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.35);
    z-index: 8;
  }
  .modal {
    position: fixed;
    z-index: 9;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(600px, calc(100vw - 2rem));
    max-height: calc(100vh - 2rem);
    display: flex;
    flex-direction: column;
    padding: 0;
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.2);
  }
  @media (max-width: 560px) {
    .modal {
      top: auto;
      bottom: 0;
      transform: translateX(-50%);
      width: 100vw;
      max-height: 92vh;
      border-radius: var(--radius) var(--radius) 0 0;
    }
  }
  header {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.8rem 1rem;
    border-bottom: 1px solid var(--surface-2);
  }
  h3 {
    margin: 0;
  }
  .emoji {
    font-size: 1.6rem;
  }
  .total {
    font-size: 1.1rem;
    white-space: nowrap;
  }
  .top {
    margin: 0;
  }
  .toggle {
    align-self: flex-start;
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .toggle button {
    border: none;
    border-radius: 0;
    font-size: 0.85rem;
    padding: 0.25rem 0.7rem;
    background: transparent;
  }
  .toggle button.active {
    background: var(--surface-2);
    font-weight: 600;
  }
  .body {
    overflow-y: auto;
    padding: 0.2rem 1rem calc(1rem + env(safe-area-inset-bottom));
  }
  .day {
    margin: 0.8rem 0 0.2rem;
    font-weight: 600;
  }
  .item {
    display: flex;
    gap: 0.75rem;
    width: 100%;
    align-items: flex-start;
    text-align: left;
    border: none;
    border-radius: 0;
    background: transparent;
    padding: 0.55rem 0.1rem;
  }
  .item + .item {
    border-top: 1px solid var(--surface-2);
  }
  .who {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .name,
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 0.8rem;
  }
  .note {
    font-size: 0.85rem;
    font-style: italic;
    color: var(--text);
    margin-top: 0.1rem;
  }
  .amt {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font-weight: 700;
    white-space: nowrap;
  }
  .amt .sub {
    font-weight: 400;
  }
</style>
