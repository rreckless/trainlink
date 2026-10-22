-- Row-level security tests. Run against a database with the migration applied, after local_stubs.sql
-- (locally) or against the real project's SQL editor (with two test users created in Authentication).
-- Each check raises an error if it fails, so a passing run ends with "RLS tests passed".

begin;

insert into auth.users (id) values
  ('00000000-0000-0000-0000-00000000000a'),
  ('00000000-0000-0000-0000-00000000000b')
on conflict do nothing;

-- Become a signed-in user, the way PostgREST does for a request with that user's token
create or replace function pg_temp.as_user(uid uuid) returns void language plpgsql as $$
begin
  perform set_config('request.jwt.claims', json_build_object('sub', uid, 'role', 'authenticated')::text, true);
  execute 'set local role authenticated';
end;
$$;

-- Become an anonymous visitor (no token)
create or replace function pg_temp.as_anon() returns void language plpgsql as $$
begin
  perform set_config('request.jwt.claims', '{"role": "anon"}', true);
  execute 'set local role anon';
end;
$$;

create or replace function pg_temp.back_to_owner() returns void language plpgsql as $$
begin
  execute 'reset role';
  perform set_config('request.jwt.claims', '', true);
end;
$$;

create or replace function pg_temp.check(ok boolean, message text) returns void language plpgsql as $$
begin
  if not coalesce(ok, false) then
    raise exception 'RLS test failed: %', message;
  end if;
  raise notice 'ok: %', message;
end;
$$;

-- True if the statement is refused, by policy, by a foreign key, or by missing privileges
create or replace function pg_temp.refused(statement text) returns boolean language plpgsql as $$
begin
  execute statement;
  return false;
exception when others then
  return true;
end;
$$;

do $$
declare
  a constant uuid := '00000000-0000-0000-0000-00000000000a';
  b constant uuid := '00000000-0000-0000-0000-00000000000b';
  layout_a uuid;
  loco_a uuid;
  block_a uuid;
  n integer;
begin
  -- User A creates a layout, a locomotive, and a block on it
  perform pg_temp.as_user(a);
  insert into public.layouts (name, width, height, scale) values ('Erinburgh', 3067, 1037, 'N') returning id into layout_a;
  insert into public.locomotives (name, dcc_address, locomotive_type) values ('Class 08 Shunter', 8, 'diesel') returning id into loco_a;
  insert into public.blocks (layout_id, name, x, y) values (layout_a, 'Platform', 10, 10) returning id into block_a;

  select count(*) into n from public.layouts;
  perform pg_temp.check(n = 1, 'A sees their own layout');
  perform pg_temp.check(pg_temp.refused($q$insert into public.layouts (name, width, height, scale) values ('Erinburgh', 1, 1, 'N')$q$),
    'A cannot make a second live layout with the same name');
  perform pg_temp.check(pg_temp.refused($q$insert into public.locomotives (name, dcc_address, locomotive_type) values ('Other', 8, 'diesel')$q$),
    'A cannot give two locomotives the same DCC address');

  -- User B sees none of A's rows
  perform pg_temp.back_to_owner();
  perform pg_temp.as_user(b);
  select count(*) into n from public.layouts;
  perform pg_temp.check(n = 0, 'B sees no layouts of A');
  select count(*) into n from public.locomotives;
  perform pg_temp.check(n = 0, 'B sees no locomotives of A');
  select count(*) into n from public.blocks;
  perform pg_temp.check(n = 0, 'B sees no blocks of A');

  -- B cannot change or remove A's rows: the update and delete touch nothing
  update public.layouts set name = 'Taken' where id = layout_a;
  get diagnostics n = row_count;
  perform pg_temp.check(n = 0, 'B cannot update A''s layout');
  delete from public.layouts where id = layout_a;
  get diagnostics n = row_count;
  perform pg_temp.check(n = 0, 'B cannot delete A''s layout');

  -- B cannot create rows for A, or link their rows to A's
  perform pg_temp.check(pg_temp.refused(format($q$insert into public.layouts (user_id, name, width, height, scale) values (%L, 'Stolen', 1, 1, 'N')$q$, a)),
    'B cannot insert a layout as A');
  perform pg_temp.check(pg_temp.refused(format($q$insert into public.blocks (layout_id, name, x, y) values (%L, 'Hijack', 0, 0)$q$, layout_a)),
    'B cannot add a block to A''s layout');
  perform pg_temp.check(pg_temp.refused(format($q$insert into public.locomotives (name, dcc_address, locomotive_type, user_id) values ('Mine', 9, 'diesel', %L)$q$, a)),
    'B cannot insert a locomotive as A');

  -- B can use the same names and addresses in their own account
  insert into public.layouts (name, width, height, scale) values ('Erinburgh', 1000, 500, 'N');
  insert into public.locomotives (name, dcc_address, locomotive_type) values ('Class 08 Shunter', 8, 'diesel');
  select count(*) into n from public.layouts;
  perform pg_temp.check(n = 1, 'B can make their own layout with the same name');

  -- A keeps their rows, unchanged by B
  perform pg_temp.back_to_owner();
  perform pg_temp.as_user(a);
  select count(*) into n from public.layouts where name = 'Erinburgh';
  perform pg_temp.check(n = 1, 'A still has their layout after B''s actions');

  -- Anonymous visitors see nothing and change nothing
  perform pg_temp.back_to_owner();
  perform pg_temp.as_anon();
  perform pg_temp.check(pg_temp.refused('select count(*) from public.layouts'), 'anonymous visitors cannot read layouts');
  perform pg_temp.check(pg_temp.refused($q$insert into public.layouts (name, width, height, scale) values ('Anon', 1, 1, 'N')$q$),
    'anonymous visitors cannot insert layouts');

  -- Pictures: A's folder is theirs alone
  perform pg_temp.back_to_owner();
  perform pg_temp.as_user(a);
  insert into storage.objects (bucket_id, name) values ('locomotive-pictures', a::text || '/' || loco_a::text || '.jpg');
  select count(*) into n from storage.objects where bucket_id = 'locomotive-pictures';
  perform pg_temp.check(n = 1, 'A sees their own picture');
  perform pg_temp.back_to_owner();
  perform pg_temp.as_user(b);
  select count(*) into n from storage.objects where bucket_id = 'locomotive-pictures';
  perform pg_temp.check(n = 0, 'B sees no pictures of A');
  perform pg_temp.check(pg_temp.refused(format($q$insert into storage.objects (bucket_id, name) values ('locomotive-pictures', %L)$q$, a::text || '/hijack.jpg')),
    'B cannot add a picture to A''s folder');

  perform pg_temp.back_to_owner();
  raise notice 'RLS tests passed';
end;
$$;

rollback;
