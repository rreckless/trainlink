-- Block borders, brake and stop points, and sensor roles, sent with each block. The core sends sync ids, as it does for the
-- other block links. Borders are two-way: each neighbour lists the block too. Roles map a sensor's sync id to entry, middle or exit.
alter table public.blocks
  add column border_block_ids jsonb not null default '[]'::jsonb,
  add column feedback_roles jsonb not null default '{}'::jsonb,
  add column brake_cm double precision,
  add column stop_cm double precision;
