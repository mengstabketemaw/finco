<script lang="ts">
  import PeriodBar from './PeriodBar.svelte'
  import TxnRow from './TxnRow.svelte'
  import type { Flow, Txn } from './api'
  import { birr, day, dayLabel, title, toSantim } from './money'
  import { inPeriod, periods } from './budget'
  import { data, ui } from './store.svelte'

  type Filter = 'all' | Flow | 'untagged' | 'unknown'
  const filters: [Filter, string][] = [
    ['all', 'All'],
    ['spend', 'Spending'],
    ['income', 'Income'],
    ['untagged', 'Untagged'],
    ['unknown', '❔ Unknown'],
    ['internal', 'Between my accounts'],
    ['loan', 'Credit'],
  ]
  let filter = $state<Filter>('all')
  let query = $state('')
  let showMore = $state(false)
  // Dates as YYYY-MM-DD (what <input type="date"> gives); amounts as typed.
  let from = $state('')
  let to = $state('')
  let min = $state('')
  let max = $state('')

  /** A date range searches every period, not just the one picked above. */
  const ranged = $derived(from !== '' || to !== '')
  const minS = $derived(toSantim(min))
  const maxS = $derived(toSantim(max))
  const extra = $derived([ranged, minS !== null, maxS !== null].filter(Boolean).length)
  /** A search that is just a number ("600", "1,500") looks for that amount. */
  const queryAmount = $derived(/^\s*[\d,]+(\.\d+)?\s*$/.test(query) ? toSantim(query) : null)

  function matchesFilter(t: Txn): boolean {
    switch (filter) {
      case 'all':
        return true
      case 'untagged':
        return t.flow === 'spend' && !t.category && t.counted > 0
      case 'unknown':
        return t.category === 'unknown'
      default:
        return t.flow === filter
    }
  }

  function matches(t: Txn): boolean {
    if (!matchesFilter(t)) return false
    if (minS !== null && t.amount < minS) return false
    if (maxS !== null && t.amount > maxS) return false
    if (queryAmount !== null) {
      // Whole birr is enough: "600" finds 600.00 and 600.50.
      return [t.amount, t.counted].some((a) => Math.floor(a / 100) === Math.floor(queryAmount / 100))
    }
    const q = query.trim().toLowerCase()
    if (!q) return true
    return [title(t), t.counterparty, t.description, t.reference, t.target_number, t.note].some((s) => s?.toLowerCase().includes(q))
  }

  function inRange(t: Txn): boolean {
    const d = day(t)
    return (!from || d >= from) && (!to || d <= to)
  }

  const all = $derived(periods(data.txns, data.plans))
  const list = $derived.by(() => {
    const p = all[Math.min(ui.period, all.length - 1)]
    return data.txns
      .filter((t) => (ranged ? inRange(t) : inPeriod(t, p)) && matches(t))
      .sort((a, b) => b.occurred_at.localeCompare(a.occurred_at))
  })
  const days = $derived.by(() => {
    const groups = new Map<string, Txn[]>()
    for (const t of list) {
      const d = day(t)
      if (!groups.has(d)) groups.set(d, [])
      groups.get(d)!.push(t)
    }
    return [...groups].map(([d, txns]) => ({
      d,
      txns,
      out: txns.reduce((s, t) => s + (t.flow === 'spend' ? t.counted : 0) + t.fee, 0),
    }))
  })
  /** Shown when anything narrows the list. */
  const narrowed = $derived(extra > 0 || query.trim() !== '' || filter !== 'all')
  const spent = $derived(list.reduce((s, t) => s + (t.flow === 'spend' ? t.counted : 0), 0))
  const received = $derived(list.reduce((s, t) => s + (t.flow === 'income' ? t.counted : 0), 0))

  function clear() {
    from = to = min = max = ''
  }
</script>

<div class="head">
  {#if ranged}
    <div class="range">
      <strong>{from ? dayLabel(from) : 'Start'} – {to ? dayLabel(to) : 'today'}</strong>
      <button class="ghost small" onclick={() => (from = to = '')} aria-label="Back to the budget period">✕</button>
    </div>
  {:else}
    <PeriodBar />
  {/if}
  <div class="searchrow">
    <input class="search" type="search" placeholder="Search names, references, amounts…" bind:value={query} />
    <button class:active={showMore || extra > 0} onclick={() => (showMore = !showMore)} aria-expanded={showMore}>
      Filters{#if extra}&nbsp;<span class="count">{extra}</span>{/if}
    </button>
  </div>
</div>

{#if showMore}
  <div class="more card">
    <label><span class="small muted">From</span><input type="date" bind:value={from} max={to || undefined} /></label>
    <label><span class="small muted">To</span><input type="date" bind:value={to} min={from || undefined} /></label>
    <label><span class="small muted">Min ETB</span><input inputmode="decimal" placeholder="0" bind:value={min} /></label>
    <label><span class="small muted">Max ETB</span><input inputmode="decimal" placeholder="any" bind:value={max} /></label>
    {#if extra}<button class="ghost small clear" onclick={clear}>Clear filters</button>{/if}
  </div>
{/if}

<div class="filters">
  {#each filters as [id, label]}
    <button class="chip" class:active={filter === id} onclick={() => (filter = id)}>{label}</button>
  {/each}
</div>

{#if narrowed && list.length}
  <p class="summary small muted">
    {[
      `${list.length} transaction${list.length === 1 ? '' : 's'}`,
      spent ? `spent ${birr(spent, true)}` : '',
      received ? `received ${birr(received, true)}` : '',
    ]
      .filter(Boolean)
      .join(' · ')}
  </p>
{/if}

{#if days.length === 0}
  <p class="empty">{data.loaded ? 'Nothing here.' : 'Loading…'}</p>
{/if}

{#each days as g (g.d)}
  <section>
    <div class="day row">
      <span>{dayLabel(g.d)}</span>
      <span class="spacer"></span>
      {#if g.out}<span class="muted small">−{birr(g.out, true)}</span>{/if}
    </div>
    <div class="card list">
      {#each g.txns as t (t.id)}
        <TxnRow {t} />
      {/each}
    </div>
  </section>
{/each}

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
  .range {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    min-width: 12rem;
  }
  .searchrow {
    flex: 1;
    display: flex;
    gap: 0.4rem;
    min-width: 12rem;
  }
  .search {
    flex: 1;
    min-width: 0;
  }
  .searchrow button.active {
    background: var(--accent-bg);
    border-color: var(--accent);
  }
  .count {
    display: inline-block;
    min-width: 1.2rem;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 0.72rem;
    line-height: 1.2rem;
    text-align: center;
  }
  /* Phones: the period centred on its own line, search full width below. */
  @media (max-width: 560px) {
    .head {
      flex-direction: column;
      align-items: stretch;
      gap: 0.5rem;
    }
  }
  .more {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 0.5rem;
    margin-top: 0.6rem;
    padding: 0.7rem;
  }
  @media (max-width: 560px) {
    .more {
      grid-template-columns: 1fr 1fr;
    }
  }
  .more label {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }
  .more input {
    width: 100%;
  }
  .clear {
    grid-column: 1 / -1;
    justify-self: start;
    padding: 0;
    color: var(--accent);
  }
  .filters {
    display: flex;
    gap: 0.35rem;
    overflow-x: auto;
    padding: 0.75rem 0 0.25rem;
    scrollbar-width: none;
  }
  .filters .chip {
    white-space: nowrap;
  }
  .summary {
    margin: 0.5rem 0.25rem 0;
  }
  .day {
    margin: 1rem 0.25rem 0.3rem;
    font-weight: 600;
    font-size: 0.9rem;
  }
  .list {
    padding: 0.15rem 0.6rem;
  }
  .list :global(.row-btn + .row-btn) {
    border-top: 1px solid var(--surface-2);
  }
</style>
