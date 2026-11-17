<script lang="ts">
  // "What was this?": the list of payments to tag. Tap one to open it, or
  // tick several and tag them together; the modal does the tagging.
  import ReviewModal from './ReviewModal.svelte'
  import type { Txn } from './api'
  import { birr, dayLabel, day, needsTag, sameRecipient, subtitle, title } from './money'
  import { data, ui } from './store.svelte'

  let sort = $state<'amount' | 'date'>('amount')
  let ticked = $state<string[]>([])
  /** Payments open in the modal. */
  let editing = $state<string[] | null>(null)
  /** How many rows to show; grows with "Show more". */
  let limit = $state(50)

  const untagged = $derived(data.txns.filter((t) => needsTag(t)))
  const unknown = $derived(data.txns.filter((t) => t.flow === 'spend' && t.category === 'unknown'))
  const pool = $derived(ui.reviewUnknown ? unknown : untagged)
  const queue = $derived(
    [...pool].sort((a, b) => (sort === 'amount' ? b.amount - a.amount : b.occurred_at.localeCompare(a.occurred_at))),
  )
  const total = $derived(queue.reduce((s, t) => s + t.amount, 0))

  /**
   * How many payments in this list go to the same recipient (or person),
   * counting itself: exactly what the modal's "Tag all … like it" offers, plus one.
   */
  const likeIt = $derived.by(() => {
    const m = new Map<string, number>()
    for (const t of pool) m.set(t.id, pool.filter((u) => sameRecipient(t, u)).length)
    return m
  })
  const times = (t: Txn) => likeIt.get(t.id) ?? 1

  // Drop ticks for payments that left the list (tagged, or on another tab).
  $effect(() => {
    const ids = new Set(queue.map((t) => t.id))
    if (ticked.some((id) => !ids.has(id))) ticked = ticked.filter((id) => ids.has(id))
  })

  function tick(id: string) {
    ticked = ticked.includes(id) ? ticked.filter((x) => x !== id) : [...ticked, id]
  }

  function switchTab(unknownTab: boolean) {
    ui.reviewUnknown = unknownTab
    ticked = []
  }

  function closed(saved: boolean) {
    if (saved) ticked = []
    editing = null
  }
  const tickedTotal = $derived(queue.filter((t) => ticked.includes(t.id)).reduce((s, t) => s + t.amount, 0))
</script>

<div class="head">
  <h2>What was this?</h2>
  {#if queue.length}
    <span class="muted">{queue.length} · ETB {birr(total, true)}</span>
  {/if}
</div>

<div class="bar">
  <div class="tabs" role="group" aria-label="Which payments">
    <button class="chip" class:active={!ui.reviewUnknown} onclick={() => switchTab(false)}>Untagged ({untagged.length})</button>
    <button class="chip" class:active={ui.reviewUnknown} onclick={() => switchTab(true)}>❔ Unknown ({unknown.length})</button>
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
      {#if unknown.length}<button onclick={() => switchTab(true)}>Look at the {unknown.length} marked Unknown</button>{/if}
    {/if}
  </div>
{:else}
  <p class="hint small muted">Tap a payment to tag it, or tick several to tag them together.</p>
  <ul class="list card">
    {#each queue.slice(0, limit) as t (t.id)}
      {@const n = times(t)}
      {@const on = ticked.includes(t.id)}
      <li class:on>
        <label class="check" aria-label="Select {title(t)}, {birr(t.amount)}">
          <input type="checkbox" checked={on} onchange={() => tick(t.id)} />
        </label>
        <button class="item" onclick={() => (editing = [t.id])}>
          <span class="who">
            <span class="name">{title(t)}{#if n > 1}<span class="times" title="{n} untagged payments like this">×{n}</span>{/if}</span>
            <span class="sub muted">{dayLabel(day(t))} · {subtitle(t, data.settings.own_numbers)}{#if t.note} · “{t.note}”{/if}</span>
          </span>
          <span class="amt">{birr(t.amount)}</span>
        </button>
      </li>
    {/each}
  </ul>
  {#if queue.length > limit}
    <button class="ghost more" onclick={() => (limit += 50)}>Show more ({queue.length - limit} left)</button>
  {/if}
{/if}

{#if ticked.length}
  <div class="selection">
    <span><strong>{ticked.length} selected</strong> <span class="muted small">· ETB {birr(tickedTotal, true)}</span></span>
    <span class="spacer"></span>
    <button class="ghost" onclick={() => (ticked = [])}>Clear</button>
    <button class="primary" onclick={() => (editing = [...ticked])}>Tag {ticked.length}</button>
  </div>
{/if}

{#if editing}
  <ReviewModal ids={editing} {pool} onclose={closed} />
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
  .tabs .chip {
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
    padding: 0.15rem 0.4rem;
    max-width: 720px;
  }
  li {
    display: flex;
    align-items: center;
  }
  li + li {
    border-top: 1px solid var(--surface-2);
  }
  li.on {
    background: var(--accent-bg);
  }
  .check {
    display: grid;
    place-items: center;
    align-self: stretch;
    padding: 0 0.45rem;
    cursor: pointer;
  }
  .check input {
    width: 1.15rem;
    height: 1.15rem;
    accent-color: var(--accent);
  }
  .item {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    border: none;
    border-radius: 0;
    background: transparent;
    padding: 0.6rem 0.3rem 0.6rem 0.1rem;
    text-align: left;
  }
  .item:hover:not(:disabled) {
    background: transparent;
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
  .more {
    margin-top: 0.5rem;
    color: var(--accent);
  }
  .done {
    font-size: 2rem;
    color: var(--in);
    margin: 0;
  }
  /* Sits above the bottom tab bar on phones. */
  .selection {
    position: sticky;
    bottom: 0.75rem;
    z-index: 4;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    max-width: 720px;
    margin-top: 0.75rem;
    padding: 0.6rem 0.8rem;
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: 0 6px 24px rgb(0 0 0 / 0.15);
  }
  @media (max-width: 560px) {
    .selection {
      bottom: 4.6rem;
    }
  }
</style>
