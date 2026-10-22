-- TrainLink cloud schema (Phase 1).
-- Every table belongs to one user. Row-level security lets a user read and write only their own rows,
-- and the composite foreign keys stop a row from pointing at another user's rows.
-- Live state (speed, direction, function states, turnout positions, running routes) is not stored here.

create or replace function public.set_updated_at()
returns trigger
language plpgsql
as $$
begin
  new.updated_at = now();
  return new;
end;
$$;

-- Layouts ------------------------------------------------------------------

create table public.layouts (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  name text not null check (char_length(trim(name)) > 0),
  description text,
  width double precision not null check (width > 0),
  height double precision not null check (height > 0),
  scale text not null,
  track_gauge text,
  elements jsonb not null default '{"tracks": []}'::jsonb,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  deleted_at timestamptz,
  unique (id, user_id)
);
create unique index layouts_user_name_live on public.layouts (user_id, name) where deleted_at is null;

-- Locomotives ----------------------------------------------------------------

create table public.locomotives (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  name text not null check (char_length(trim(name)) > 0),
  dcc_address integer not null check (dcc_address between 1 and 9999),
  locomotive_type text not null,
  length double precision,
  function_labels jsonb,
  picture_path text,
  travelled_time double precision not null default 0,
  travelled_distance double precision not null default 0,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  deleted_at timestamptz,
  unique (id, user_id)
);
create unique index locomotives_user_name_live on public.locomotives (user_id, name) where deleted_at is null;
create unique index locomotives_user_address_live on public.locomotives (user_id, dcc_address) where deleted_at is null;

-- Command stations (no screen uses them yet; kept so locomotives can refer to one) ---

create table public.command_stations (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  name text not null check (char_length(trim(name)) > 0),
  station_type text not null,
  port text not null,
  settings jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  deleted_at timestamptz,
  unique (id, user_id)
);

alter table public.locomotives
  add column command_station_id uuid,
  add constraint locomotives_command_station_fk
    foreign key (command_station_id, user_id) references public.command_stations (id, user_id);

-- Blocks, signals, feedbacks, routes ---------------------------------------------

create table public.blocks (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  layout_id uuid not null,
  name text not null,
  x double precision not null,
  y double precision not null,
  max_speed integer not null default 100,
  length double precision,
  state text not null default 'free' check (state in ('free', 'occupied', 'reserved')),
  occupancy_locomotive_id uuid,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  deleted_at timestamptz,
  unique (id, user_id),
  foreign key (layout_id, user_id) references public.layouts (id, user_id) on delete cascade,
  foreign key (occupancy_locomotive_id, user_id) references public.locomotives (id, user_id)
);

create table public.signals (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  layout_id uuid not null,
  name text not null,
  x double precision not null,
  y double precision not null,
  signal_type text not null check (signal_type in ('main', 'warning')),
  aspect text not null default 'stop' check (aspect in ('stop', 'caution', 'go')),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  deleted_at timestamptz,
  unique (id, user_id),
  foreign key (layout_id, user_id) references public.layouts (id, user_id) on delete cascade
);

create table public.feedbacks (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  layout_id uuid not null,
  name text not null,
  x double precision not null,
  y double precision not null,
  feedback_number integer not null,
  state boolean not null default false,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  deleted_at timestamptz,
  unique (id, user_id),
  foreign key (layout_id, user_id) references public.layouts (id, user_id) on delete cascade
);

create table public.routes (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  layout_id uuid not null,
  name text not null,
  block_sequence jsonb not null default '[]'::jsonb,
  speed_profile jsonb,
  dwell_times jsonb,
  auto_return boolean not null default false,
  enabled boolean not null default true,
  current_locomotive_id uuid,
  current_block_index integer not null default 0,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  deleted_at timestamptz,
  unique (id, user_id),
  foreign key (layout_id, user_id) references public.layouts (id, user_id) on delete cascade,
  foreign key (current_locomotive_id, user_id) references public.locomotives (id, user_id)
);

create table public.block_signals (
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  block_id uuid not null,
  signal_id uuid not null,
  primary key (block_id, signal_id),
  foreign key (block_id, user_id) references public.blocks (id, user_id) on delete cascade,
  foreign key (signal_id, user_id) references public.signals (id, user_id) on delete cascade
);

create table public.block_feedbacks (
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  block_id uuid not null,
  feedback_id uuid not null,
  primary key (block_id, feedback_id),
  foreign key (block_id, user_id) references public.blocks (id, user_id) on delete cascade,
  foreign key (feedback_id, user_id) references public.feedbacks (id, user_id) on delete cascade
);

-- Moving time: one event per run, added up, so devices never overwrite each other ---

create table public.moving_time_events (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  locomotive_id uuid not null,
  device_id text not null,
  seconds double precision not null check (seconds > 0),
  recorded_at timestamptz not null default now(),
  foreign key (locomotive_id, user_id) references public.locomotives (id, user_id) on delete cascade
);

-- Updated-at stamps ----------------------------------------------------------------

create trigger layouts_updated_at before update on public.layouts for each row execute function public.set_updated_at();
create trigger locomotives_updated_at before update on public.locomotives for each row execute function public.set_updated_at();
create trigger command_stations_updated_at before update on public.command_stations for each row execute function public.set_updated_at();
create trigger blocks_updated_at before update on public.blocks for each row execute function public.set_updated_at();
create trigger signals_updated_at before update on public.signals for each row execute function public.set_updated_at();
create trigger feedbacks_updated_at before update on public.feedbacks for each row execute function public.set_updated_at();
create trigger routes_updated_at before update on public.routes for each row execute function public.set_updated_at();

-- Row-level security: each user sees and changes only their own rows --------------

do $$
declare
  t text;
begin
  foreach t in array array[
    'layouts', 'locomotives', 'command_stations', 'blocks', 'signals', 'feedbacks', 'routes',
    'block_signals', 'block_feedbacks', 'moving_time_events'
  ]
  loop
    execute format('alter table public.%I enable row level security', t);
    execute format(
      'create policy "own rows" on public.%I for all to authenticated using (user_id = auth.uid()) with check (user_id = auth.uid())',
      t
    );
  end loop;
end;
$$;

-- Anonymous visitors get nothing, and nothing is granted to them ------------------------

revoke all on all tables in schema public from anon;
grant usage on schema public to anon, authenticated;
grant select, insert, update, delete on all tables in schema public to authenticated;

grant usage on schema storage to authenticated;
grant select, insert, update, delete on storage.objects to authenticated;

-- Pictures: a private bucket, with each user's pictures in their own folder -----------

insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values ('locomotive-pictures', 'locomotive-pictures', false, 2097152, array['image/jpeg', 'image/png', 'image/webp'])
on conflict (id) do nothing;

create policy "own pictures" on storage.objects
  for all to authenticated
  using (bucket_id = 'locomotive-pictures' and (storage.foldername(name))[1] = auth.uid()::text)
  with check (bucket_id = 'locomotive-pictures' and (storage.foldername(name))[1] = auth.uid()::text);
