-- Settings: the Z21 address and any setting the app keeps for the account. One row per setting, shared by every
-- device signed in to the account. The newer change wins, as for the other tables.

create table public.settings (
  id uuid primary key,
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  name text not null check (char_length(trim(name)) > 0),
  value jsonb not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  deleted_at timestamptz,
  unique (user_id, name)
);

alter table public.settings enable row level security;

create policy "own rows" on public.settings for all to authenticated
  using (user_id = auth.uid()) with check (user_id = auth.uid());

grant select, insert, update, delete on public.settings to authenticated;
