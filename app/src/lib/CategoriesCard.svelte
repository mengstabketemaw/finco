<script lang="ts">
  // Settings: your categories. Rename, change the emoji, add, remove.
  // Removing hides a category from the pickers; payments already tagged
  // with it keep it, and it can be brought back.
  import { FIXED, cats, newId, type Category, type CategoryLists } from './categories.svelte'
  import { api } from './api'
  import { data, reload, toast } from './store.svelte'

  let kind = $state<'spend' | 'income'>('spend')
  let lists = $state<CategoryLists>({ spend: [], income: [] })
  let emoji = $state('')
  let name = $state('')

  // Edit a copy; Save writes it.
  $effect(() => {
    lists = { spend: cats.spend.map((c) => ({ ...c })), income: cats.income.map((c) => ({ ...c })) }
  })

  const shown = $derived(lists[kind].filter((c) => !c.hidden))
  const removed = $derived(lists[kind].filter((c) => c.hidden))
  /** How many payments use each category (so removing says what stays). */
  const uses = $derived.by(() => {
    const m = new Map<string, number>()
    for (const t of data.txns) {
      for (const id of new Set([t.category, ...t.splits.map((p) => p.category)])) if (id) m.set(id, (m.get(id) ?? 0) + 1)
    }
    return m
  })
  const changed = $derived(JSON.stringify(lists) !== JSON.stringify({ spend: cats.spend, income: cats.income }))

  function setHidden(c: Category, hidden: boolean) {
    c.hidden = hidden
  }

  function add() {
    const label = name.trim()
    if (!label) return
    const id = newId(label)
    lists[kind].push({ id, label, emoji: emoji.trim() || '•' })
    name = emoji = ''
  }

  function move(c: Category, by: number) {
    const list = lists[kind]
    const i = list.indexOf(c)
    const j = i + by
    if (j < 0 || j >= list.length) return
    ;[list[i], list[j]] = [list[j], list[i]]
  }

  async function save() {
    const clean = (l: Category[]) =>
      l.filter((c) => c.label.trim()).map((c) => ({ ...c, label: c.label.trim(), emoji: c.emoji.trim() || '•' }))
    const snap = $state.snapshot(lists)
    await api.saveSettings({ ...$state.snapshot(data.settings), categories: { spend: clean(snap.spend), income: clean(snap.income) } })
    await reload()
    toast('Categories saved')
  }
</script>

<section class="card stack">
  <h3>Categories</h3>
  <p class="small muted">What you pick from when tagging. Removing one only hides it: payments already tagged keep it.</p>

  <div class="toggle" role="group" aria-label="Which categories">
    <button class:active={kind === 'spend'} onclick={() => (kind = 'spend')}>Spending ({lists.spend.filter((c) => !c.hidden).length})</button>
    <button class:active={kind === 'income'} onclick={() => (kind = 'income')}>Income ({lists.income.filter((c) => !c.hidden).length})</button>
  </div>

  <ul class="cats">
    {#each shown as c, i (c.id)}
      <li>
        <input class="emoji" bind:value={c.emoji} maxlength="4" aria-label="{c.label} emoji" />
        <input class="name" bind:value={c.label} aria-label="Name" />
        <span class="uses muted small" title="Payments tagged with it">{uses.get(c.id) ?? 0}</span>
        <button class="ghost small" onclick={() => move(c, -1)} disabled={i === 0} aria-label="Move {c.label} up">↑</button>
        {#if FIXED.has(c.id)}
          <span class="fixed muted small" title="The app needs this one; you can rename it">built-in</span>
        {:else}
          <button class="ghost small" onclick={() => setHidden(c, true)} aria-label="Remove {c.label}">✕</button>
        {/if}
      </li>
    {/each}
  </ul>

  <div class="add">
    <input class="emoji" bind:value={emoji} placeholder="🙂" maxlength="4" aria-label="New category emoji" />
    <input class="name" bind:value={name} placeholder="New category" onkeydown={(e) => e.key === 'Enter' && add()} aria-label="New category name" />
    <button onclick={add} disabled={!name.trim()}>Add</button>
  </div>

  {#if removed.length}
    <div class="removed">
      <span class="small muted">Removed (old payments keep them):</span>
      {#each removed as c (c.id)}
        <button class="chip" onclick={() => setHidden(c, false)} title="Bring back">{c.emoji} {c.label} · bring back</button>
      {/each}
    </div>
  {/if}

  <div class="row">
    <button class="primary" disabled={!changed} onclick={save}>Save</button>
    {#if changed}<span class="small muted">Not saved yet</span>{/if}
  </div>
</section>

<style>
  p {
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
    padding: 0.3rem 0.8rem;
    background: transparent;
  }
  .toggle button.active {
    background: var(--surface-2);
    font-weight: 600;
  }
  .cats {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  li,
  .add {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }
  .emoji {
    width: 3rem;
    flex: none;
    text-align: center;
  }
  .name {
    flex: 1;
    min-width: 0;
  }
  .uses {
    min-width: 1.6rem;
    text-align: right;
  }
  .fixed {
    width: 3.6rem;
    text-align: center;
    font-size: 0.72rem;
  }
  li .ghost:last-child {
    width: 3.6rem;
  }
  .removed {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    align-items: center;
  }
</style>
