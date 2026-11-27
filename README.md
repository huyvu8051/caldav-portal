# caldav-portal

Account portal for [`caldav-rs`](https://github.com/huyvu8051/caldav-rs) and
`notion-bridge`: sign in, get CalDAV credentials, connect Notion, link a
database to a calendar.

It runs **beside** `notion-caldav-saas` rather than replacing it. Customers
move only once this is stable, and that migration is its own piece of work.

## What it owns, and what it does not

The portal owns accounts, sessions and links. It does **not** own calendar
data and never reaches into another service's tables -- the same boundary
`notion-bridge` keeps.

Two consequences that look like extra work and are not:

- **Creating a CalDAV account goes through `caldav-rs`'s provisioning
  endpoint**, not a direct `INSERT`. Different database, and Postgres does not
  write across one. It also keeps the password-hash format inside the service
  that verifies it.
- **Creating a calendar does not.** CalDAV already has `MKCALENDAR`, and the
  portal uses it the way any other client would.

## The password lives in two places

`caldav-rs` holds an argon2 hash, because it is the side that verifies a
login. This portal holds the plaintext, because it has to show the password
again and a hash does not come back.

They are not two sources of truth: the plaintext here is the source and the
hash there is derived from it. They can drift exactly one way -- running
`caldav-rs adduser` by hand with a new password changes the hash there and
tells nobody. There is a warning beside that command.

This is a deliberate, temporary choice. The alternative is rotate-only: show
the password once, store nothing, and lose the ability to show it again.

## Running it

Needs Postgres, a Keycloak client, and `caldav-rs` with its provisioning
listener enabled.

```sh
cargo leptos watch
```

Configuration, all required except where noted:

| | |
|---|---|
| `DATABASE_URL` | Postgres for accounts and sessions |
| `KEYCLOAK_ISSUER_URL` | e.g. `https://auth.opendiy.vn/realms/notion-caldav-saas` |
| `KEYCLOAK_CLIENT_ID` | defaults to `caldav-portal-app` |
| `KEYCLOAK_CLIENT_SECRET` | |
| `PORTAL_BASE_URL` | the externally reachable origin; the OIDC redirect is built from it, and it decides whether the session cookie is `Secure` |
| `CALDAV_ADMIN_URL` | `caldav-rs`'s provisioning listener |
| `CALDAV_ADMIN_TOKEN` | must match its `ADMIN_TOKEN` |
| `CALDAV_PUBLIC_URL` | what a person types into their phone |

The process refuses to start if any of these is missing. That is on purpose:
a portal that starts against whatever happened to be in the environment is a
portal that provisions accounts somewhere nobody intended.

## Design

Server-rendered and hydrated, like `webcaldav`, and sharing its design
system -- tokens, `ui.rs`, and the suite that enforces them. The UI is
phone-shaped and stays phone-shaped: on a desktop the same 430px column is
centred and the background expands around it. There is no second layout to
keep in step with the first.
