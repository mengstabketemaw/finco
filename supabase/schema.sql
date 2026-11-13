-- finco sync schema. Run in the Supabase dashboard: SQL Editor → New query →
-- paste → Run. Safe to run again after an update.
--
-- The tables mirror the local SQLite tables, plus:
--   user_id    the owner; row-level security limits every row to its owner
--   synced_at  set by the server on every accepted write; devices pull rows
--              newer than the last synced_at they saw
-- Writes are last-write-wins by updated_at: an upsert carrying an older or
-- equal updated_at than the stored row is silently skipped.
--
-- Only parsed fields of bank messages are stored (amount, balance, masked
-- account numbers, names, references), never the raw SMS text.

create or replace function public.finco_lww() returns trigger
language plpgsql as $$
begin
  if tg_op = 'UPDATE' and new.updated_at <= old.updated_at then
    return null;
  end if;
  new.synced_at := clock_timestamp();
  return new;
end
$$;

create table if not exists public.messages (
  user_id     uuid    not null default auth.uid() references auth.users on delete cascade,
  id          text    not null,
  received_at bigint  not null,
  parsed      text    not null,
  updated_at  bigint  not null,
  deleted     integer not null default 0,
  synced_at   timestamptz not null default clock_timestamp(),
  primary key (user_id, id)
);

create table if not exists public.txn_meta (
  user_id     uuid    not null default auth.uid() references auth.users on delete cascade,
  id          text    not null,
  category    text,
  note        text,
  settles     text,
  updated_at  bigint  not null,
  deleted     integer not null default 0,
  synced_at   timestamptz not null default clock_timestamp(),
  primary key (user_id, id)
);

-- Added later: which earlier transactions a repayment settles.
alter table public.txn_meta add column if not exists settles text;

create table if not exists public.recipient_rules (
  user_id     uuid    not null default auth.uid() references auth.users on delete cascade,
  key         text    not null,
  category    text    not null,
  updated_at  bigint  not null,
  deleted     integer not null default 0,
  synced_at   timestamptz not null default clock_timestamp(),
  primary key (user_id, key)
);

create table if not exists public.settings (
  user_id     uuid    not null default auth.uid() references auth.users on delete cascade,
  key         text    not null,
  value       text    not null,
  updated_at  bigint  not null,
  deleted     integer not null default 0,
  synced_at   timestamptz not null default clock_timestamp(),
  primary key (user_id, key)
);

do $$
declare
  t text;
begin
  foreach t in array array['messages', 'txn_meta', 'recipient_rules', 'settings']
  loop
    execute format('create index if not exists %I on public.%I (user_id, synced_at)', t || '_synced', t);
    execute format('alter table public.%I enable row level security', t);
    execute format('drop policy if exists "own rows" on public.%I', t);
    execute format(
      'create policy "own rows" on public.%I for all to authenticated
         using (user_id = auth.uid()) with check (user_id = auth.uid())', t);
    execute format('drop trigger if exists finco_lww on public.%I', t);
    execute format(
      'create trigger finco_lww before insert or update on public.%I
         for each row execute function public.finco_lww()', t);
    execute format('grant select, insert, update on public.%I to authenticated', t);
    -- Realtime: devices hear about each other's changes at once.
    if not exists (
      select 1 from pg_publication_tables
      where pubname = 'supabase_realtime' and schemaname = 'public' and tablename = t
    ) then
      execute format('alter publication supabase_realtime add table public.%I', t);
    end if;
  end loop;
end
$$;
