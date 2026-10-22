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
2. Under Authentication → Users, add two test users with auto-confirm on, and note their emails and passwords.
3. Run the HTTP tests with the public anon key, not the secret key:

```
SUPABASE_URL=https://bnvvdicbimmmcmllease.supabase.co \
SUPABASE_ANON_KEY=<anon key> \
TEST_A_EMAIL=<test user A> TEST_A_PASSWORD=<password> \
TEST_B_EMAIL=<test user B> TEST_B_PASSWORD=<password> \
node --test supabase/tests/rls_http_test.mjs
```

Keep the test passwords out of the repository. The secret key is never needed for these tests.
