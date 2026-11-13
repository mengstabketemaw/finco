<script lang="ts">
  import { appOnly, type SmsInfo, type SyncInfo } from './api'
  import { dayLabel } from './money'
  import { reload, toast } from './store.svelte'

  let info = $state<SyncInfo | null>(null)
  let sms = $state<SmsInfo | null>(null)
  let url = $state('')
  let key = $state('')
  let email = $state('')
  let password = $state('')
  let busy = $state(false)
  let error = $state('')
  let showUnrecognized = $state(false)

  async function load() {
    info = await appOnly.syncStatus()
    sms = await appOnly.smsInfo()
    url ||= info.default_url
    key ||= info.default_key
  }

  $effect(() => {
    load()
  })

  async function login(signup: boolean) {
    busy = true
    error = ''
    try {
      toast(await appOnly.syncLogin(url, key, email, password, signup))
      password = ''
      await load()
      await reload()
    } catch (e) {
      error = explain(String(e))
    } finally {
      busy = false
    }
  }

  /** Supabase's message plus what to do about it. */
  function explain(msg: string): string {
    if (/Invalid login credentials/i.test(msg)) return `${msg}. No account with this email and password: check them, or tap Create account first.`
    if (/Email not confirmed/i.test(msg)) return `${msg}. Click the link in the email Supabase sent, then log in. (Or turn off "Confirm email" in Supabase → Authentication → Email.)`
    if (/already registered/i.test(msg)) return `${msg}. Tap Log in instead.`
    if (/rate limit/i.test(msg)) return `${msg}. Supabase only sends a few emails an hour; wait, or turn off "Confirm email" in Supabase → Authentication → Email.`
    if (/can't reach/i.test(msg)) return `${msg}. Check the internet connection and the project URL.`
    return msg
  }

  async function logout() {
    await appOnly.syncLogout()
    await load()
  }
</script>

<section class="card stack">
  <h3>Messages</h3>
  {#if sms}
    <p class="small muted">
      {sms.messages} bank and wallet messages stored.
      {#if sms.reads_sms}New ones are read as they arrive; the inbox is checked each time the app opens.{:else}This computer gets them from your phone through sync.{/if}
    </p>
    {#if sms.unrecognized.length}
      <button class="ghost link small" onclick={() => (showUnrecognized = !showUnrecognized)}>
        {sms.unrecognized.length} looked like money but weren't understood {showUnrecognized ? '▴' : '▾'}
      </button>
      {#if showUnrecognized}
        <ul class="unrec">
          {#each sms.unrecognized as u}
            <li><span class="muted small">{u.sender} · {dayLabel(new Date(u.received_at + 3 * 3600_000).toISOString())}</span><br />{u.body}</li>
          {/each}
        </ul>
      {/if}
    {/if}
  {/if}
</section>

<section class="card stack">
  <h3>Sync</h3>
  {#if info?.status.account}
    <p class="small">
      Syncing as <strong>{info.status.account}</strong>.
      {#if info.status.pending}{info.status.pending} changes waiting to send.{:else}Everything sent.{/if}
    </p>
    <div><button onclick={logout}>Log out</button></div>
  {:else if info}
    <p class="small muted">Log in to keep the phone and this computer in step. Use the same account on both.</p>
    <label class="field"><span class="small muted">Project URL</span><input bind:value={url} placeholder="https://….supabase.co" /></label>
    <label class="field"><span class="small muted">Publishable key</span><input bind:value={key} /></label>
    <label class="field"><span class="small muted">Email</span><input type="email" autocomplete="username" bind:value={email} /></label>
    <label class="field"><span class="small muted">Password</span><input type="password" autocomplete="current-password" bind:value={password} /></label>
    {#if error}<p class="error small">{error}</p>{/if}
    <div class="row">
      <button class="primary" disabled={busy} onclick={() => login(false)}>{busy ? 'Working…' : 'Log in'}</button>
      <button disabled={busy} onclick={() => login(true)}>Create account</button>
    </div>
  {/if}
</section>

<style>
  p {
    margin: 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .link {
    padding: 0;
    color: var(--accent);
    text-align: left;
  }
  .error {
    color: var(--danger);
  }
  .unrec {
    margin: 0;
    padding-left: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    font-size: 0.85rem;
    overflow-wrap: anywhere;
  }
</style>
