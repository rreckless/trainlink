# Cloud data (Supabase)

The schema, row-level security, and picture storage for TrainLink's sign-in and cloud data.

- `migrations/20261022100000_init.sql`: the tables (UUID keys, one `user_id` per row), the row-level security policies, and the private `locomotive-pictures` bucket.
- `tests/rls_test.sql`: row-level security checks in plain Postgres, with two users and an anonymous visitor.
- `tests/local_stubs.sql`: stand-ins for Supabase's `auth` and `storage` pieces, used only by the plain-Postgres tests.
- `tests/rls_http_test.mjs`: the same checks through the real Supabase API.

## Test without touching the project

```
docker run -d --name tl-rls-test -e POSTGRES_PASSWORD=localtest -p 5499:5432 postgres:16-alpine
docker exec -i tl-rls-test psql -U postgres -v ON_ERROR_STOP=1 < supabase/tests/local_stubs.sql
docker exec -i tl-rls-test psql -U postgres -v ON_ERROR_STOP=1 < supabase/migrations/20261022100000_init.sql
docker exec -i tl-rls-test psql -U postgres -v ON_ERROR_STOP=1 < supabase/tests/rls_test.sql
docker rm -f tl-rls-test
```

A passing run ends with `RLS tests passed`.

## Apply to the project

1. In the Supabase dashboard, open the SQL Editor and paste the contents of `migrations/20261022100000_init.sql`. Run it once.
   Then run each later migration in order. `migrations/20261026100000_block_links.sql` must be run before a version that syncs block links is used, or sync will be refused.
2. Under Authentication → Users, add two test users with auto-confirm on, and note their emails and passwords.
3. Run the HTTP tests with the public anon key, not the secret key. Put the settings in a file outside the repository, for example `~/trainlink-test.env`:

```
export SUPABASE_URL=https://bnvvdicbimmmcmllease.supabase.co
export SUPABASE_ANON_KEY=<anon key>
export TEST_A_EMAIL=<test user A>
export TEST_A_PASSWORD=<password>
export TEST_B_EMAIL=<test user B>
export TEST_B_PASSWORD=<password>
```

Then, from the repository root, in the same terminal:

```
set -a; source ~/trainlink-test.env; set +a
node --test supabase/tests/rls_http_test.mjs
```

The test lists any setting that is missing. Keep the file and the passwords out of the repository. The secret key is never needed for these tests.
