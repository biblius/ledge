# Ledgeknaw

A ledger for your knawledge.

Built with love in rust and svelte.

Live example available [here](https://knawledge.josipbenkodjakovic.from.hr).

## Development

Requirements:

- Rust and its tooling ([install here](https://www.rust-lang.org/tools/install))
- vite (`npm i -g vite`)
- postgres ([install here](https://www.postgresql.org/download/))

The backend requires you set the `DATABASE_URL` environment variable in the shell from which you invoke cargo and a database called `knaw`.

```bash
export DATABASE_URL=postgresql://me:mypassword@127.0.0.1:5432/knaw
```

To start the application:

1. From the project root run

```bash
cargo run [ -- -l INFO|DEBUG|TRACE]
```

2. In another shell, head to the [web](web) directory and execute

```bash
npm run dev
```

3. Go to http://127.0.0.1:3030 and ingest knawledge.
