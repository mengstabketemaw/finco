<script lang="ts">
  // Money back for a payment: pick the money in that paid it back. Credits
  // are sorted by how far they are in time from the payment, before or
  // after, and more load as you scroll.
  import { untrack } from 'svelte'
  import { api, type Txn } from './api'
  import { birr, dayLabel, day, time, title, toSantim } from './money'
  import { data, reload, toast } from './store.svelte'

  let { t, onclose }: { t: Txn; onclose: () => void } = $props()

  const DAY = 864e5
  const PAGE = 30
  const at = (x: Txn) => Date.parse(x.occurred_at)

  // Ticked when opened: the money in already linked to this payment.
  let chosen = $state<string[]>(untrack(() => data.txns.filter((x) => x.settles.includes(t.id)).map((x) => x.id)))
  let query = $state('')
  let limit = $state(PAGE)
  let busy = $state(false)
  let sentinel = $state<HTMLElement>()
  let scroller = $state<HTMLElement>()

  /** Money in that could be a payback: not borrowed money, not salary. */
  const credits = $derived(
    data.txns
      .filter((x) => x.flow === 'income' && x.id !== t.id && !['borrowed', 'salary'].includes(x.category ?? ''))
      .map((x) => ({ x, gap: at(x) - at(t) }))
      .sort((a, b) => Math.abs(a.gap) - Math.abs(b.gap)),
  )
  const queryAmount = $derived(/^\s*[\d,]+(\.\d+)?\s*$/.test(query) ? toSantim(query) : null)
  const matching = $derived.by(() => {
    const q = query.trim().toLowerCase()
    if (!q) return credits
    return credits.filter(({ x }) =>
      queryAmount !== null
        ? Math.floor(x.amount / 100) === Math.floor(queryAmount / 100)
        : [title(x), x.counterparty, x.note, x.description].some((s) => s?.toLowerCase().includes(q)),
    )
  })
  const shown = $derived(matching.slice(0, limit))

  // A new search starts from the nearest again.
  $effect(() => {
    void query
    limit = PAGE
    if (scroller) scroller.scrollTop = 0
  })

  // Load the next page when the end of the list scrolls into view.
  $effect(() => {
    if (!sentinel || !scroller) return
    const io = new IntersectionObserver(
      (es) => {
        if (es.some((e) => e.isIntersecting)) limit += PAGE
      },
      { root: scroller, rootMargin: '200px' },
    )
    io.observe(sentinel)
    return () => io.disconnect()
  })

  function gapLabel(gap: number): string {
    const d = Math.round(Math.abs(gap) / DAY)
    const side = gap >= 0 ? 'after' : 'before'
    if (d === 0) {
      const m = Math.round(Math.abs(gap) / 6e4)
      if (m === 0) return 'same time'
      return m < 60 ? `${m} min ${side}` : `${Math.round(m / 60)} h ${side}`
    }
    return `${d} day${d === 1 ? '' : 's'} ${side}`
  }

  /** Other payments a credit already pays back. */
  const elsewhere = (x: Txn) =>
    x.settles
      .filter((id) => id !== t.id)
      .map((id) => data.txns.find((p) => p.id === id))
      .filter((p): p is Txn => !!p)

  function toggle(id: string) {
    chosen = chosen.includes(id) ? chosen.filter((c) => c !== id) : [...chosen, id]
  }

  const back = $derived(data.txns.filter((x) => chosen.includes(x.id)).reduce((s, x) => s + x.amount, 0))
  const share = $derived(Math.max(0, t.amount - back))

  async function save() {
    busy = true
    try {
      await api.setRepaidBy(t.id, chosen)
      await reload()
      toast(chosen.length ? `Linked ${chosen.length}: your share is now ${birr(Math.max(0, t.counted))}` : 'Unlinked')
      onclose()
    } finally {
      busy = false
    }
  }
</script>

<div class="backdrop" onclick={onclose} role="presentation"></div>
<div class="modal card" role="dialog" aria-modal="true" aria-label="Money back">
  <header>
    <div class="row">
      <div>
        <h3>↩ Money back</h3>
        <span class="muted small">for {title(t)} · {dayLabel(day(t))} · ETB {birr(t.amount)}</span>
      </div>
      <span class="spacer"></span>
      <button class="ghost" onclick={onclose} aria-label="Close">✕</button>
    </div>
    <input type="search" bind:value={query} placeholder="Search name or amount" />
    <span class="small muted">Nearest in time first, before or after. Tick everything that paid it back.</span>
  </header>

  <div class="list" bind:this={scroller}>
    {#each shown as { x, gap } (x.id)}
      {@const other = elsewhere(x)}
      <label class="item" class:on={chosen.includes(x.id)}>
        <input type="checkbox" checked={chosen.includes(x.id)} onchange={() => toggle(x.id)} />
        <span class="who">
          <span class="name">{title(x)}</span>
          <span class="sub muted">
            {dayLabel(day(x))} · {time(x)} · <strong>{gapLabel(gap)}</strong>
            {#if other.length}· also pays back {title(other[0])}{other.length > 1 ? ` +${other.length - 1}` : ''}{/if}
          </span>
        </span>
        <span class="amt">+{birr(x.amount)}</span>
      </label>
    {/each}
    {#if shown.length < matching.length}
      <div class="end muted small" bind:this={sentinel}>Loading more…</div>
    {:else}
      <div class="end muted small">{matching.length ? 'That was all of them.' : 'No money in like that.'}</div>
    {/if}
  </div>

  <footer class="row">
    <span class="small">
      {#if chosen.length}
        <strong>{chosen.length} picked · ETB {birr(back, true)} back</strong><br /><span class="muted">your share ≈ ETB {birr(share, true)}</span>
      {:else}
        <span class="muted">Nothing picked</span>
      {/if}
    </span>
    <span class="spacer"></span>
    <button onclick={onclose}>Cancel</button>
    <button class="primary" disabled={busy} onclick={save}>{busy ? 'Saving…' : 'Save'}</button>
  </footer>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.35);
    z-index: 30;
  }
  .modal {
    position: fixed;
    z-index: 31;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(560px, calc(100vw - 2rem));
    height: min(720px, calc(100vh - 2rem));
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
      height: 92vh;
      border-radius: var(--radius) var(--radius) 0 0;
    }
  }
  header,
  footer {
    flex: none;
    padding: 0.8rem 1rem;
  }
  header {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    border-bottom: 1px solid var(--surface-2);
  }
  h3 {
    margin: 0;
  }
  header input {
    width: 100%;
  }
  footer {
    border-top: 1px solid var(--surface-2);
    padding-bottom: calc(0.8rem + env(safe-area-inset-bottom));
  }
  .list {
    flex: 1;
    overflow-y: auto;
    padding: 0.2rem 0.6rem;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.55rem 0.3rem;
    cursor: pointer;
  }
  .item + .item {
    border-top: 1px solid var(--surface-2);
  }
  .item.on {
    background: var(--accent-bg);
  }
  .item input {
    width: 1.15rem;
    height: 1.15rem;
    flex: none;
    accent-color: var(--accent);
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
  .amt {
    font-weight: 700;
    color: var(--in);
    white-space: nowrap;
  }
  .end {
    text-align: center;
    padding: 0.8rem 0 1rem;
  }
</style>
