<script lang="ts">
  import CategoryGrid from './CategoryGrid.svelte'
  import Settle from './Settle.svelte'
  import { api } from './api'
  import { INCOME_CATEGORIES, SOURCES, SPEND_CATEGORIES, birr, category, dayLabel, day, recipientKey, time, title } from './money'
  import { categorize, data, reload, ui } from './store.svelte'

  const t = $derived(data.txns.find((t) => t.id === ui.open))
  const pair = $derived(t?.pair_id ? data.txns.find((p) => p.id === t.pair_id) : undefined)
  let remember = $state(true)
  let note = $state('')

  $effect(() => {
    note = t?.note ?? ''
  })

  function close() {
    ui.open = null
  }

  async function saveNote() {
    if (t && note !== (t.note ?? '')) {
      await api.setNote(t.id, note)
      await reload()
    }
  }

  const flowText: Record<string, string> = {
    internal: 'Moved between your own accounts. Not counted as spending or income.',
    loan: "Credit movement. The payment it covered is counted on its own, so this isn't counted again.",
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

{#if t}
  <div class="backdrop" onclick={close} role="presentation"></div>
  <div class="sheet card" role="dialog" aria-modal="true" aria-label="Transaction">
    <div class="row">
      <span class="muted small">{dayLabel(day(t))} · {time(t)} · {SOURCES[t.source] ?? t.source}</span>
      <span class="spacer"></span>
      <button class="ghost" onclick={close} aria-label="Close">✕</button>
    </div>
    <div class="amount" class:in={t.flow === 'income'}>
      {t.flow === 'income' ? '+' : t.flow === 'spend' ? '−' : ''}ETB {birr(t.amount)}
    </div>
    <div class="who">{title(t)}</div>

    {#if flowText[t.flow]}
      <p class="note-box small">{flowText[t.flow]}</p>
    {/if}

    <Settle {t} />

    <!-- A repayment has no category of its own: it just cancels out. -->
    {#if (t.flow === 'spend' || t.flow === 'income') && t.settles.length === 0}
      <h3>Category {#if t.category}<span class="muted small">· {category(t.category).label}</span>{/if}</h3>
      <CategoryGrid
        categories={t.flow === 'spend' ? SPEND_CATEGORIES : INCOME_CATEGORIES}
        selected={t.category}
        onpick={(id) => categorize(t, id, remember && t.flow === 'spend' && recipientKey(t) !== null)}
      />
      {#if t.flow === 'spend' && recipientKey(t)}
        <label class="row small remember"><input type="checkbox" bind:checked={remember} /> Use this for every payment to {title(t)}</label>
      {/if}
    {/if}

    <label class="field">
      <span class="small muted">Note</span>
      <input bind:value={note} onblur={saveNote} placeholder="What was it for?" />
    </label>

    <dl class="small">
      {#if t.fee}<dt>Fees</dt><dd>ETB {birr(t.fee)}</dd>{/if}
      {#if t.balance !== null}<dt>Balance after</dt><dd>ETB {birr(t.balance)}</dd>{/if}
      {#if t.outstanding !== null}<dt>Credit owed after</dt><dd>ETB {birr(t.outstanding)}</dd>{/if}
      {#if t.counterparty_account}<dt>Their account</dt><dd>{t.counterparty_account}</dd>{/if}
      {#if t.target_number}<dt>Number</dt><dd>{t.target_number}</dd>{/if}
      {#if t.description && t.description !== title(t)}<dt>Details</dt><dd>{t.description}</dd>{/if}
      {#if t.reference}<dt>Reference</dt><dd class="ref">{t.reference}</dd>{/if}
      {#if pair}
        <dt>Other side</dt>
        <dd><button class="ghost link" onclick={() => (ui.open = pair.id)}>{SOURCES[pair.source]} · {time(pair)}</button></dd>
      {/if}
    </dl>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.35);
    z-index: 10;
  }
  .sheet {
    position: fixed;
    z-index: 11;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(560px, calc(100vw - 2rem));
    max-height: calc(100vh - 2rem);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.2);
  }
  /* Phones: a bottom sheet. */
  @media (max-width: 560px) {
    .sheet {
      top: auto;
      bottom: 0;
      transform: translateX(-50%);
      width: 100vw;
      max-height: 88vh;
      border-radius: var(--radius) var(--radius) 0 0;
    }
  }
  .amount {
    font-size: 1.8rem;
    font-weight: 700;
    line-height: 1.1;
  }
  .who {
    font-size: 1.05rem;
  }
  .note-box {
    background: var(--surface-2);
    border-radius: 6px;
    padding: 0.5rem 0.7rem;
    margin: 0.25rem 0;
  }
  h3 {
    margin-top: 0.5rem;
  }
  .remember {
    margin-top: 0.1rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    margin-top: 0.5rem;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.3rem 1rem;
    margin: 0.5rem 0 0;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    text-align: right;
    overflow-wrap: anywhere;
  }
  .ref {
    font-family: ui-monospace, monospace;
  }
  .link {
    padding: 0;
    color: var(--accent);
  }
</style>
