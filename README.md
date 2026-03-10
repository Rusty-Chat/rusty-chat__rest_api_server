# Krabby Chat Core REST API Server

[![CI](https://github.com/KrabbyHQ/chat__core_rest_api_server/actions/workflows/ci.yml/badge.svg)](https://github.com/KrabbyHQ/chat__core_rest_api_server/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Conventional Commits](https://img.shields.io/badge/Conventional%20Commits-1.0.0-yellow.svg)](https://conventionalcommits.org)
[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.1-4baaaa.svg)](CODE_OF_CONDUCT.md)

This repository contains the CORE REST API service for the Krabby `chat` implementation.

## Core Features

- Domain-driven API modules: `user`, `admin`, `rooms`, and `messages`.
- PostgreSQL persistence with SQLx-based query execution.
- AWS S3 integration for profile images, room images, and message attachments.
- JWT-based session/auth flow with cookie deployment.
- Multi-layer configuration system (`config/*.toml` + `APP__*` env overrides).
- Middleware-based request pipeline (sessions, access control, admin protection, request logging, timeout).
- Structured startup validation for required configuration sections.

## Setup and Execution

### 1. Core Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable).
- [Docker](https://www.docker.com/).
- [sqlx-cli](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli) (`cargo install sqlx-cli`).
- [Node.js](https://nodejs.org/en/download/) and [Bun](https://bun.sh/) for contribution standards tooling.

### 2. Install Dependencies

```shell
git clone https://github.com/KrabbyHQ/chat__core_rest_api_server.git
cd chat__core_rest_api_server
cargo build
```

### 3. Database Setup

Start a local PostgreSQL container:

```shell
docker run -d --name <container-name> -p 5433:5432 -e POSTGRES_USER=<user-name> -e POSTGRES_PASSWORD=<password> -e POSTGRES_DB=<database-name> postgres
```

Example:

```shell
docker run -d --name rusty-chat__dev_db -p 5433:5432 -e POSTGRES_USER=okpainmo -e POSTGRES_PASSWORD=supersecret -e POSTGRES_DB=rusty_chat_db_dev postgres
```

Run migrations:

```shell
sqlx migrate run --database-url postgres://<user-name>:<password>@localhost:5433/<database-name>
```

Example:

```shell
sqlx migrate run --database-url postgres://okpainmo:supersecret@localhost:5433/rusty_chat_db_dev
```

#### If Contributing New Schema Changes

1. Create a migration:

```shell
sqlx migrate add <migration_name>
```

E.g.

```shell
sqlx migrate add added_new_hello_field_to_users_table
```

2. Edit the migration file to add the SQL schema update.

3. Sync the database with the new migration.

```shell
sqlx migrate run --database-url postgres://<user-name>:<password>@localhost:5433/<database-name>
```

E.g.

```shell
sqlx migrate run --database-url postgres://okpainmo:supersecret@localhost:5433/rusty_chat_db_dev
```

### 4. Running the Server

*Ensure to have installed `cargo-watch`.*

```shell
cargo install cargo-watch
```

To start the server in development mode(auto-reload enabled), simply run:

```shell
cargo dev
```

> `cargo-watch` handles the server/project reloads on-save. See `.config/config.toml` for reference on the `dev` command.

*Note: The `dev` command is an alias for `cargo watch`. If you are on WSL and reload doesn't trigger, proceed to use the polling command option(also see `.cargo/config.toml` for reference on that).*

### 5. Contribution Standards Tooling

This repository uses Husky and Commitlint(via Bun) to enforce commit conventions and basic workflow checks. Follow the instructions below, to ensure your local setup is updated with checks to help your contributions meet the standards.

1. Ensure to sync with the main branch and pull in all updates first:

```shell
git pull origin main
```

2. Install the new packages with Bun:

```shell
bun install
```

## Project Configuration Setup

The project uses a centralized config model built around `src/utils/load_config.rs`.

### Loading Order (Lowest to Highest Precedence)

1. `config/base.toml`
2. `config/{APP__ENV}.toml` (for example `development`, `staging`, `production`)
3. `config/local.toml` (optional local override)
4. Environment variables prefixed with `APP__`

### Mapping Rule for Environment Variables

Double underscore (`__`) maps nested TOML keys.

Syntax:

`APP__<SECTION>__<FIELD>=value`

Example:

```toml
[server]
port = 8000
```

Override via:

`APP__SERVER__PORT=9000`

### Single Source of Truth Rule

Runtime settings are consumed through `AppConfig` from `load_config()`.

Direct runtime env reads are intentionally avoided in application flow (outside config/bootstrap loaders), so config remains the canonical source for server, database, auth, and AWS values.

### Mandatory Config Sections

Startup validation enforces:

- `app`
- `server`
- `database`
- `auth`
- `aws`

## Environment Files

The repository includes `.sample` files to help bootstrap your local setup.

### Recommended Setup

- Copy `.env.sample` to `.env`
- Copy `.env.development.sample` to `.env.development`

### Available Files

- `.env`: selects deployment profile (`APP__DEPLOY__ENV`).
- `.env.development`: development overrides.
- `.env.staging`: staging overrides.
- `.env.production`: production overrides.

Do not commit real secrets.

## API Surface Overview

Base prefix: `/api/v1`

### User Routes (`/user`)

- `GET /get-user/{user_id}`
- `GET /get-all-users`
- `PATCH /update-user/{user_id}`
- `PATCH /update-password/{user_id}`
- `PATCH /update-profile-image/{user_id}`

### Admin Routes (`/admin`)

- `PATCH /add-admin/{user_id}`
- `PATCH /remove-admin/{user_id}`
- `PATCH /activate-user/{user_id}`
- `PATCH /deactivate-user/{user_id}`

### Rooms Routes (`/rooms`)

- Room creation/update/fetch endpoints
- Membership/admin management endpoints
- Bookmark/pin/archive room state endpoints
- Listing endpoints (all, private, group, open, closed, by-user)

### Messages Routes (`/messages`)

- Message create/update/delete endpoints
- Bookmark/archive endpoints
- Edit-history and status receipt endpoints
- Delivery/seen sync endpoints
- Reaction endpoint

For exact paths and method bindings, see:

- `src/domains/user/router.rs`
- `src/domains/admin/router.rs`
- `src/domains/rooms/router.rs`
- `src/domains/messages/router.rs`

## Testing

### Current State

There is no dedicated `tests/` directory in the current repository state.

### Recommended Commands

Run all tests (including inline module tests):

```shell
cargo test
```

Run compile verification:

```shell
cargo check
```

### Adding New Tests

- Unit tests: add `#[cfg(test)]` module blocks to target files.
- Integration tests: add files under a root-level `tests/` directory and validate full request flows.

## Reliability and Robustness Notes

- Database pooling is configured through `database.max_connections` and `database.connect_timeout_secs`.
- Cookie deployment is environment-aware (`Secure` in non-development).
- Startup fails fast when required config sections/fields are missing.
- Request timeout middleware currently uses a 60-second timeout.
- Request logging middleware captures path and request duration.

## Logging Layers

1. Global logging middleware for request lifecycle timing.
2. In-process error logs inside handlers/middlewares for runtime diagnostics.

## Operating System Notes (WSL)

If file watch behavior is inconsistent on WSL, switch the dev alias in `.cargo/config.toml` to the polling-based `cargo watch` command.

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting changes.

### Code of Conduct

Please review [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Security

Please report vulnerabilities according to [SECURITY.md](SECURITY.md).

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE).
