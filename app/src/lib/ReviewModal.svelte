<script lang="ts">
  // Tag one or more payments: pick a category, write a note, Save.
  // Other untagged payments to the same recipient are listed (by date) and
  // can be included with one switch.
  import { untrack } from 'svelte'
  import CategorySelect from './CategorySelect.svelte'
  import CreditPicker from './CreditPicker.svelte'
  import Settle from './Settle.svelte'
  import SplitEditor from './SplitEditor.svelte'
  import type { Txn } from './api'
  import { birr, spendCategories, category as categoryOf, dayLabel, day, recipientKey, sameRecipient, subtitle, time, title } from './money'
  import { data, tagMany, ui } from './store.svelte'

  let {
    ids,
    pool,
    onclose,
  }: {
    /** The payments picked on the list. */
    ids: string[]
    /** The list they came from (untagged or Unknown), for finding similar ones. */
    pool: Txn[]
    onclose: (saved: boolean) => void
  } = $props()

  const picked = $derived(ids.map((id) => data.txns.find((t) => t.id === id)).filter((t): t is Txn => !!t))
  /** Recipients of the picked payments (those that have one). */
  const keys = $derived(new Set(picked.map(recipientKey).filter((k): k is string => k !== null)))
  /** Same recipient (or same person), still in the list, not picked. */
  const similar = $derived(
    pool
      .filter((t) => !ids.includes(t.id) && picked.some((p) => sameRecipient(p, t)))
      .sort((a, b) => b.occurred_at.localeCompare(a.occurred_at)),
  )
  const similarByDay = $derived.by(() => {
    const groups = new Map<string, Txn[]>()
    for (const t of similar) {
      const d = day(t)
      if (!groups.has(d)) groups.set(d, [])
      groups.get(d)!.push(t)
    }
    return [...groups]
  })

  // Filled in once when the modal opens: a sync arriving while you type
  // must not reset your choice.
  const first = untrack(() => picked)
  let chosen = $state<string | null>(first.length === 1 ? first[0].category : null)
  let includeSimilar = $state(false)
  let remember = $state(true)
  let busy = $state(false)
  // One payment: its note, editable. Several: empty means "leave their notes".
  let note = $state(first.length === 1 ? (first[0].note ?? '') : '')

  const targets = $derived(includeSimilar ? [...picked, ...similar] : picked)
  const total = $derived(targets.reduce((s, t) => s + t.amount, 0))
  const oneRecipient = $derived(keys.size === 1 && picked.every((t) => recipientKey(t) !== null))
  const noteChanged = $derived(picked.length === 1 ? note !== (picked[0].note ?? '') : note.trim() !== '')
  const canSave = $derived(!busy && (chosen !== null || noteChanged))
  /** One payment also gets "Money back", like in its detail sheet. */
  const actions = $derived(picked.length === 1 ? [{ id: 'money_back', label: '↩ Money back…', run: () => (picking = true) }] : [])
  /** The "Money back" picker is open (one payment only). */
  let picking = $state(false)

  /** Everything else about the payment: the full detail sheet. */
  function details() {
    const id = picked[0].id
    onclose(false)
    ui.open = id
  }

  async function save() {
    busy = true
    try {
      await tagMany(
        targets.map((t) => t.id),
        chosen,
        noteChanged ? note.trim() : null,
        remember && oneRecipient,
      )
      onclose(true)
    } finally {
      busy = false
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (picking ? (picking = false) : onclose(false))} />

<div class="backdrop" onclick={() => onclose(false)} role="presentation"></div>
<div class="modal card" role="dialog" aria-modal="true" aria-label="Tag payments">
  <header class="row">
    <div>
      <h3>{picked.length === 1 ? title(picked[0]) : `${picked.length} payments`}</h3>
      <span class="muted small">
        {#if picked.length === 1}{dayLabel(day(picked[0]))} · {subtitle(picked[0], data.settings.own_numbers)}{:else}picked on the list{/if}
      </span>
    </div>
    <span class="spacer"></span>
    <button class="ghost" onclick={() => onclose(false)} aria-label="Close">✕</button>
  </header>

  <div class="body">
    {#if picked.length === 1}
      <div class="big">ETB {birr(picked[0].amount)}</div>
      {#if picked[0].description && picked[0].kind === 'payment'}<p class="muted small">{picked[0].description}</p>{/if}
    {:else}
      <ul class="lines">
        {#each picked as t (t.id)}
          <li><span>{title(t)} <span class="muted small">{dayLabel(day(t))}</span></span><span>{birr(t.amount)}</span></li>
        {/each}
      </ul>
    {/if}

    {#if similar.length}
      <section class="similar">
        <label class="switch">
          <input type="checkbox" bind:checked={includeSimilar} />
          <span>
            <strong>Tag all {similar.length} like {picked.length === 1 ? 'it' : 'them'} too</strong><br />
            <span class="small muted">Other payments to {keys.size === 1 ? title(similar[0]) : 'the same people'} still in this list</span>
          </span>
        </label>
        <div class="groups" class:on={includeSimilar}>
          {#each similarByDay as [d, ts] (d)}
            <div class="day small muted">{dayLabel(d)}</div>
            {#each ts as t (t.id)}
              <div class="line small"><span>{title(t)} · {time(t)}</span><span>{birr(t.amount)}</span></div>
            {/each}
          {/each}
        </div>
      </section>
    {/if}

    <h4>What was it?</h4>
    <CategorySelect categories={spendCategories()} value={chosen} onchange={(id) => (chosen = id)} {actions} />

    <label class="field">
      <span class="small muted">Note{picked.length > 1 || includeSimilar ? ' (added to all of them; empty leaves their notes)' : ''}</span>
      <textarea rows="2" bind:value={note} placeholder="What was it for? Who with?"></textarea>
    </label>

    {#if oneRecipient && chosen && !['lent', 'unknown'].includes(chosen)}
      <label class="row small">
        <input type="checkbox" bind:checked={remember} />
        Use {categoryOf(chosen).label} for future payments to {title(picked[0])}
      </label>
    {/if}

    {#if picked.length === 1}
      <!-- Saved right away, like in the detail sheet: money back, splits, lent. -->
      <Settle t={picked[0]} />
      {#if picked[0].settles.length === 0 && !['lent', 'written_off'].includes(picked[0].category ?? '')}
        <SplitEditor t={picked[0]} />
      {/if}
      <button class="ghost small details" onclick={details}>Open full details ›</button>
    {/if}
  </div>

  <footer class="row">
    <span class="muted small">{targets.length} payment{targets.length === 1 ? '' : 's'} · ETB {birr(total, true)}</span>
    <span class="spacer"></span>
    <button onclick={() => onclose(false)}>Cancel</button>
    <button class="primary" disabled={!canSave} onclick={save}>{busy ? 'Saving…' : 'Save'}</button>
  </footer>
</div>

{#if picking && picked.length === 1}
  <CreditPicker t={picked[0]} onclose={() => (picking = false)} />
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.35);
    z-index: 10;
  }
  .modal {
    position: fixed;
    z-index: 11;
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
  /* Phones: a sheet from the bottom, nearly full height. */
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
  header,
  footer {
    padding: 0.8rem 1rem;
    flex: none;
  }
  header {
    border-bottom: 1px solid var(--surface-2);
    align-items: flex-start;
  }
  footer {
    border-top: 1px solid var(--surface-2);
    padding-bottom: calc(0.8rem + env(safe-area-inset-bottom));
  }
  .body {
    overflow-y: auto;
    padding: 0.8rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }
  h4 {
    margin: 0.2rem 0 0;
    font-size: 0.95rem;
  }
  p {
    margin: 0;
  }
  .big {
    font-size: 1.7rem;
    font-weight: 700;
  }
  .lines {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .lines li,
  .line {
    display: flex;
    justify-content: space-between;
    gap: 0.75rem;
  }
  .similar {
    background: var(--surface-2);
    border-radius: 8px;
    padding: 0.6rem 0.75rem;
  }
  .switch {
    display: flex;
    gap: 0.6rem;
    align-items: flex-start;
  }
  .switch input {
    margin-top: 0.25rem;
    accent-color: var(--accent);
  }
  .groups {
    margin-top: 0.5rem;
    max-height: 11rem;
    overflow-y: auto;
    opacity: 0.6;
  }
  .groups.on {
    opacity: 1;
  }
  .day {
    margin-top: 0.35rem;
    font-weight: 600;
  }
  .details {
    align-self: flex-start;
    padding: 0;
    color: var(--accent);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  textarea {
    font: inherit;
    color: inherit;
    border: 1px solid var(--border);
    background: var(--surface);
    border-radius: 6px;
    padding: 0.4rem 0.55rem;
    resize: vertical;
  }
  textarea:focus {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
</style>
