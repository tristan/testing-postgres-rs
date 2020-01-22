# testing_postgres
![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

A tool for starting postgresql instances for testing.

Automatically setups a postgresql instance in a temporary directory, and destroys it after use.

## Example

```
use testing_postgres;
use postgres;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = testing_postgres::PsqlServer::start()?;
    let mut client = postgres::Client::connect(
        &format!("host=localhost user=postgres dbname=test port={}", server.port),
        postgres::NoTls)?;

    client.simple_query("
        CREATE TABLE person (
            id      SERIAL PRIMARY KEY,
            name    TEXT NOT NULL,
            data    BYTEA
        )
    ")?;

    let name = "Ferris";
    let data = None::<&[u8]>;
    client.execute(
        "INSERT INTO person (name, data) VALUES ($1, $2)",
        &[&name, &data],
    )?;

    for row in client.query("SELECT id, name, data FROM person", &[])? {
        let id: i32 = row.get(0);
        let name: &str = row.get(1);
        let data: Option<&[u8]> = row.get(2);

        println!("found person: {} {} {:?}", id, name, data);
    }

    Ok(())
}
```
