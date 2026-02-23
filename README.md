# Rusty Chat.

A modular(domain-driven-development patterned) CORE REST API layer/service for the Krabby `chat` implementation.

## Features

> ... in progress.

## How to run the project/server.

1. Clone this repository.

```shell
git clone https://github.com/KrabbyHQ/chat__core_rest_api_server.git
```

2. Install all the dependencies(and compile code-base).

```shell
cargo build
```

or install latest versions individually

```shell
cargo add argon2 aws-config --features behavior-version-latest aws-sdk-s3 --features behavior-version-latest axum --features multipart chrono --features serde,clock dotenvy jsonwebtoken --features rust_crypto rand serde --features derive serde_json sqlx --features postgres,runtime-tokio-native-tls,chrono time tokio --features full tower-cookies tracing tracing-subscriber --features fmt,json,time aws-credential-types --features hardcoded-credentials anyhow config uuid --features v4
```

3. Start the local database via Docker

E.g.

```shell
docker run -d --name rusty-chat__dev_db -p 5433:5432 -e POSTGRES_USER=okpainmo -e POSTGRES_PASSWORD=supersecret -e POSTGRES_DB=rusty_chat_db_dev postgres
```

3. Run the server

```shell
cargo run # single session - no auto-refresh on file save
```

Or

- Install `cargo-watch`

```shell
cargo install cargo-watch
```

- Run with custom server start alias/command

```shell
cargo dev
```

> The above command runs the server with `cargo-watch`. See the alias/command config inside `.cargo.config.toml`. Basically, it does the same thing as below.

```shell
cargo watch -q -c -w src/ -x "run"
```

> **Skip Step 4: It only applies during extra/progressive development**

4. Prepare to sync SQL schema with database.

```shell
sqlx migrate add <migration_name>
```

E.g.

```shell
sqlx migrate add init
```

5. Migrate(Sync with the DB)

```shell
sqlx migrate run --database-url <database-url>
```

E.g.

```shell
sqlx migrate run --database-url postgres://okpainmo:supersecret@localhost:5433/rusty_chat_db_dev
```

## Operating System Issues - Including WSL.

To avoid any issues related to the operating system, it is recommended to use a build from a Linux environment.

> WSL(Windows Sub-system For Linux) works just fine in case you're building from a Windows environment.

Cheers!!!

