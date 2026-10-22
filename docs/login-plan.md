# Login and cloud data plan

Plan for sign-in and cloud storage, so a person sees the same layouts, locomotives, and pictures in every version of TrainLink: Mac, Windows, web (local), iOS, and Android. For review. Planning only; nothing has been built.

## Decisions

| Area | Decision |
|------|----------|
| Login and data service | Supabase (Auth and Postgres, with Storage for pictures) |
| Sign-in methods | Email and password, Sign in with Apple, Google |
| Data region | Europe. Pick London or Frankfurt when the project is created |
| Web version | Local only for now: the core on the Mac serves it. A hosted web version comes later |
| Remote train control | Out of scope. Trains can be driven only from a device on the same network as the Z21 |
| Plan | Supabase free plan for now. No automatic backups on the free plan, and projects pause after a week of inactivity. Upgrade later |
| Apple sign-in | Needs an Apple Developer account, which is not yet enrolled. Add it when enrolled |

## What the user sees

- Signing in on any device shows the same layouts, locomotives, pictures, and function labels.
- Editing on one device appears on the others after they sync.
- The app works without a connection once it has signed in: it uses its local copy, and syncs when it reconnects.
- Driving trains still needs the Z21 on the same network. Away from home, the app shows its data but the driving controls say the Z21 is not reachable.

## Architecture

- **Supabase** holds the data and the sign-in. Each table has a `user_id`, and row-level security policies stop anyone reading or writing another person's rows, even with a valid key.
- **Each app keeps its local SQLite database** as its working copy. Every change goes to the local database first, then into an outbox, which is sent to Supabase when there's a connection.
- **The Rust core owns sync.** The UI sends changes to the core, and the core handles the outbox, pulls changes, and stores the session. The web version uses the same core through the local server.
- **Sessions are stored in the operating system's keychain** on the Mac (Keychain), Windows (Credential Manager), iOS (Keychain), and Android (Keystore). The web version stores its session in the browser, with the least access it needs.
- **Pictures** go to a per-user folder in Supabase Storage. The app shrinks each picture before upload.

## Data changes needed

- **Record IDs become UUIDs.** Integer IDs from different devices would clash. Your existing records keep their names, but their IDs change to UUIDs. Keep a mapping table so your old IDs can be traced.
- **Every table gets `user_id`, `updated_at`, and `deleted_at`.** Deletions are kept as tombstones so they sync.
- **References use UUIDs too.** For example, a block's occupying locomotive.
- **Moving time is stored as events,** such as "42 seconds on this device", and added up. This stops two devices overwriting each other's totals.
- **Conflicts:** when two devices change the same record, the later change wins. This is simple, and it can lose an edit if two devices change the same layout at the same moment. The rule can be made finer later.

## Sign-in flows

- **Email:** sign up with a confirmation email, sign in, password reset. Password rules follow Supabase's defaults, which we tighten.
- **Apple and Google:** sign in through the provider, then Supabase creates or links the account. Once Apple is enrolled, Sign in with Apple is also required on iOS.
- **Sign-out** clears the keychain entry and the local data for that account. The local data is removed so the next person on the device doesn't see it.
- **Account deletion** removes the account, its data, and its pictures. It must be available inside the app, which the App Store requires.
- **Sessions** refresh in the background. An expired session asks for sign-in again, and changes made while signed out stay in the outbox.

## Your existing data

- Sign in once, and an import step claims the existing local data (Erinburgh and Class 08 Shunter, with its picture) into your account.
- Erinburgh is now layout ID 6 in the old database, not 3 as in your notes. The import matches by name and content.
- The old Postgres database is left untouched until you confirm the import.

## Security

- **Row-level security** on every table. Each policy has a test that signs in as two people and checks that neither sees the other's data.
- **Keys:** the public key goes in the app. The secret service key never goes in the repository or the apps.
- **Email confirmation** is required before sign-in.
- **Rate limits** on sign-in and sign-up, from Supabase, with a check of the settings.
- **Local web server** keeps its localhost-only setting. Signing in does not make it reachable from the network, and the network token from the security plan is still needed before that.
- The security plan's other items still apply, including the origin checks and the importer fix.

## Phases

1. **Supabase project and schema.** Create the project in Europe, define the tables with UUIDs and row-level security, and write the two-account tests. You create the project and send me its URL and public key.
2. **Mac app: sign-in and sync.** Sign-in screens, keychain storage, outbox, pull and merge, and the import of your data. Verify with two Mac copies and an offline test.
3. **Local web on the same sync.** The browser signs in through the local server, and the core syncs.
4. **Windows.** The same core and sign-in, using Credential Manager.
5. **Android, then iOS.** Native sign-in, keychain or keystore storage, and the same sync. iOS waits for the Apple Developer account.
6. **Hosted web (later).** A static site that signs in and works on data only.

Phases 1 to 3 can be reviewed and tested before any mobile work starts.

## Risks

- **Migration:** a wrong import would lose or duplicate your layout. The old database stays until you check the import.
- **Free plan limits:** pausing after inactivity, no backups, and the free storage and bandwidth allowances. The export tool gives a manual backup meanwhile.
- **Conflict loss:** the simple rule can drop an edit when two devices change the same layout at once.
- **Row-level security mistakes:** a mistake could show one person's data to another. The two-account tests are the defence, and they run before any other phase.
- **Apple and Google setup:** each needs its own developer account or OAuth client, and they can take time to approve.
- **Offline gaps:** a device that has never signed in can't use the app without a connection.

## Open items

- Create the Supabase project (Europe) and send me its URL and public key.
- Decide the Google sign-in setup: I can create the OAuth client with you.
- Enrol in the Apple Developer programme when ready.
- Confirm the email sender for confirmation emails. The Supabase default is limited, so a custom sender may be needed later.
