-- Block links, sent with each block: the block after it, and the feedbacks and signals it uses. The core sends sync ids,
-- the way routes send their block sequence. The join tables from the first migration are not used by the app any more.
alter table public.blocks
  add column next_block_id uuid,
  add column feedback_ids jsonb not null default '[]'::jsonb,
  add column signal_ids jsonb not null default '[]'::jsonb;

alter table public.blocks
  add constraint blocks_next_block_fk foreign key (next_block_id, user_id) references public.blocks (id, user_id);
