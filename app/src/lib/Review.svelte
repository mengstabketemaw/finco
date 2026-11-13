<script lang="ts">
  // "What was this?": pick a payment from the list, then say what it was.
  // Sorted biggest first (a few answers explain most of the money) or
  // newest first (recent ones are easiest to remember).
  import CategoryGrid from './CategoryGrid.svelte'
  import type { Txn } from './api'
  import { SOURCES, SPEND_CATEGORIES, birr, dayLabel, day, recipientKey, subtitle, title } from './money'
  import { categorize, data, ui } from './store.svelte'

  let remember = $state(true)
  let sort = $state<'amount' | 'date'>('amount')
  let selected = $state<string | null>(null)
  /** How many rows to show; grows with "Show more". */
  let limit = $state(40)

  const untagged = $derived(data.txns.filter((t) => t.flow === 'spend' && !t.category && t.counted > 0))
  const unknown = $derived(data.txns.filter((t) => t.flow === 'spend' && t.category === 'unknown'))
  const queue = $derived(
    [...(ui.reviewUnknown ? unknown : untagged)].sort((a, b) =>
      sort === 'amount' ? b.amount - a.amount : b.occurred_at.localeCompare(a.occurred_at),
    ),
  )
  const total = $derived(queue.reduce((s, t) => s + t.amount, 0))
  /** Unknown is a category of its own, so it isn't offered as an answer here. */
  const choices = SPEND_CATEGORIES.filter((c) => c.id !== 'unknown')

  /** How often you've paid each recipient, all time: names you pay often are easier to place. */
  const timesPaid = $derived.by(() => {
    const m = new Map<string, number>()
    for (const t of data.txns) {
      if (t.flow !== 'spend') continue
      const k = recipientKey(t)
      if (k) m.set(k, (m.get(k) ?? 0) + 1)
    }
    return m
  })
  const times = (t: Txn) => {
    const k = recipientKey(t)
    return k ? (timesPaid.get(k) ?? 1) : 1
  }
  /** Other payments here to the same recipient, tagged along when remembering. */
  const alike = (t: Txn) => {
    const k = recipientKey(t)
    return k ? queue.filter((o) => o.id !== t.id && recipientKey(o) === k).length : 0
  }

  /** Answer, then open the next one in the list. */
  async function answer(t: Txn, category: string, rememberIt: boolean) {
    const i = queue.findIndex((x) => x.id === t.id)
    await categorize(t, category, rememberIt)
    const next = queue.filter((x) => x.id !== t.id)[i] ?? queue[queue.length - 1]
    selected = next && next.id !== t.id ? next.id : null
    remember = true
  }

  function toggle(t: Txn) {
    selected = selected === t.id ? null : t.id
    remember = true
  }
</script>

<div class="head">
  <h2>What was this?</h2>
  {#if queue.length}
    <span class="muted">{queue.length} · ETB {birr(total, true)}</span>
  {/if}
</div>

<div class="bar">
  <div class="tabs" role="group" aria-label="Which payments">
    <button class="chip" class:active={!ui.reviewUnknown} onclick={() => ((ui.reviewUnknown = false), (selected = null))}>Untagged ({untagged.length})</button>
    <button class="chip" class:active={ui.reviewUnknown} onclick={() => ((ui.reviewUnknown = true), (selected = null))}>❔ Unknown ({unknown.length})</button>
  </div>
  <div class="toggle" role="group" aria-label="Sort">
    <button class:active={sort === 'amount'} onclick={() => (sort = 'amount')}>Biggest</button>
    <button class:active={sort === 'date'} onclick={() => (sort = 'date')}>Newest</button>
  </div>
</div>

{#if !data.loaded}
  <p class="empty">Loading…</p>
{:else if queue.length === 0}
  <div class="card empty">
    {#if ui.reviewUnknown}
      <p>Nothing marked Unknown.</p>
    {:else}
      <p class="done" aria-hidden="true">✓</p>
      <p>Everything is tagged.</p>
      {#if unknown.length}<button onclick={() => (ui.reviewUnknown = true)}>Look at the {unknown.length} marked Unknown</button>{/if}
    {/if}
  </div>
{:else}
  <p class="hint small muted">Tap a payment, then pick what it was.</p>
  <ul class="list card">
    {#each queue.slice(0, limit) as t (t.id)}
      {@const open = selected === t.id}
      {@const n = times(t)}
      <li class:open>
        <button class="item" onclick={() => toggle(t)} aria-expanded={open}>
          <span class="who">
            <span class="name">{title(t)}{#if n > 1}<span class="times" title="You paid them {n} times">×{n}</span>{/if}</span>
            <span class="sub muted">{dayLabel(day(t))} · {subtitle(t, data.settings.own_numbers)}</span>
          </span>
          <span class="amt">{birr(t.amount)}</span>
        </button>
        {#if open}
          <div class="panel">
            {#if t.description && t.kind === 'payment'}<p class="muted small">{t.description}</p>{/if}
            <CategoryGrid categories={choices} onpick={(id) => answer(t, id, remember && recipientKey(t) !== null)} />
            <div class="row actions">
              {#if recipientKey(t)}
                {@const more = alike(t)}
                <label class="row small">
                  <input type="checkbox" bind:checked={remember} />
                  Always for {title(t)}{#if more}&nbsp;<span class="muted">(+{more} more here)</span>{/if}
                </label>
              {:else}
                <span class="muted small">{SOURCES[t.source]} doesn't say who this went to, so it can't be remembered.</span>
              {/if}
              <span class="spacer"></span>
              {#if !ui.reviewUnknown}
                <button class="ghost" onclick={() => answer(t, 'unknown', false)} title="Files it under Unknown, to look at later">Don't know</button>
              {/if}
            </div>
          </div>
        {/if}
      </li>
    {/each}
  </ul>
  {#if queue.length > limit}
    <button class="ghost more" onclick={() => (limit += 40)}>Show more ({queue.length - limit} left)</button>
  {/if}
{/if}

<style>
  .head {
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    margin-bottom: 0.6rem;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-bottom: 0.5rem;
  }
  .tabs {
    display: flex;
    gap: 0.35rem;
    flex: 1;
  }
  .tabs .chip,
  .actions button {
    white-space: nowrap;
  }
  .toggle {
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
  .hint {
    margin: 0 0 0.4rem;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0.15rem 0.6rem;
    max-width: 720px;
  }
  li + li {
    border-top: 1px solid var(--surface-2);
  }
  .item {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    border: none;
    border-radius: 0;
    background: transparent;
    padding: 0.6rem 0.2rem;
    text-align: left;
  }
  li.open .item {
    font-weight: 600;
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
    font-weight: 400;
  }
  .times {
    display: inline-block;
    margin-left: 0.4rem;
    padding: 0 0.4rem;
    border-radius: 999px;
    background: var(--accent-bg);
    color: var(--accent);
    font-size: 0.75rem;
    font-weight: 700;
    vertical-align: 0.1em;
  }
  .amt {
    font-weight: 700;
    white-space: nowrap;
  }
  .panel {
    padding: 0.2rem 0 0.8rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .panel p {
    margin: 0;
  }
  .actions {
    flex-wrap: nowrap;
  }
  .more {
    margin-top: 0.5rem;
    color: var(--accent);
  }
  .done {
    font-size: 2rem;
    color: var(--in);
    margin: 0;
  }
</style>
