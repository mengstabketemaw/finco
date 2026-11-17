<script lang="ts">
  // Pick a category from a dropdown (the phone shows its own list), with
  // "New category…" to add one on the spot, and optional actions at the end
  // such as "Money back…".
  import { category, type Category } from './money'
  import { addCategory } from './store.svelte'

  let {
    categories,
    value,
    onchange,
    kind = 'spend',
    actions = [],
    none = 'Pick a category…',
    label = 'Category',
  }: {
    categories: Category[]
    value: string | null
    /** null: "none" picked. */
    onchange: (id: string | null) => void
    kind?: 'spend' | 'income'
    /** Shown after the categories; picking one runs it and leaves the value as it was. */
    actions?: { id: string; label: string; run: () => void }[]
    none?: string
    label?: string
  } = $props()

  const NEW = '__new'
  let adding = $state(false)
  let emoji = $state('')
  let name = $state('')
  let busy = $state(false)

  /** The current value may be a removed category: keep it showing. */
  const current = $derived(value && !categories.some((c) => c.id === value) ? value : null)

  function picked(e: Event) {
    const el = e.currentTarget as HTMLSelectElement
    const id = el.value
    const action = actions.find((a) => a.id === id)
    if (id === NEW || action) {
      el.value = value ?? ''
      if (action) action.run()
      else adding = true
      return
    }
    onchange(id || null)
  }

  async function add() {
    if (!name.trim()) return
    busy = true
    try {
      const id = await addCategory(kind, name, emoji)
      adding = false
      name = emoji = ''
      onchange(id)
    } finally {
      busy = false
    }
  }
</script>

<select class="pick" value={value ?? ''} onchange={picked} aria-label={label}>
  <option value="">{none}</option>
  {#each categories as c (c.id)}<option value={c.id}>{c.emoji} {c.label}</option>{/each}
  {#if current}<option value={current} disabled>{category(current).emoji} {category(current).label} (removed)</option>{/if}
  <option value={NEW}>＋ New category…</option>
  {#each actions as a (a.id)}<option value={a.id}>{a.label}</option>{/each}
</select>

{#if adding}
  <div class="new">
    <input class="emoji" bind:value={emoji} placeholder="🙂" maxlength="4" aria-label="Emoji" />
    <!-- svelte-ignore a11y_autofocus -->
    <input class="name" bind:value={name} placeholder="New category name" autofocus onkeydown={(e) => e.key === 'Enter' && add()} aria-label="Name" />
    <button class="primary small" disabled={!name.trim() || busy} onclick={add}>Add</button>
    <button class="ghost small" onclick={() => (adding = false)} aria-label="Cancel">✕</button>
  </div>
{/if}

<style>
  .pick {
    width: 100%;
    font-size: 1rem;
    padding: 0.5rem 0.6rem;
  }
  .new {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    margin-top: 0.4rem;
  }
  .emoji {
    width: 3.2rem;
    flex: none;
    text-align: center;
  }
  .name {
    flex: 1;
    min-width: 0;
  }
</style>
