use std::process::{Command, Child, Stdio};
use std::net::{SocketAddrV4, Ipv4Addr, TcpListener};
use std::thread;
use std::time::Duration;
use std::fmt;
use std::fs;

use tempdir::TempDir;

fn which(command: &str) -> Result<String, ()> {
    let mut cmd = if cfg!(target_os = "windows") {
        Command::new("where")
    } else {
        Command::new("which")
    };
    let output = cmd.arg(command).output()
        .expect("failed to execute `which`");

    if output.status.success() {
        let s = String::from_utf8(output.stdout)
            .map_err(|_| ())?;
        Ok(s.trim().to_owned())
    } else {
        Err(())
    }
}

fn get_unused_port() -> Result<u16, std::io::Error> {
    let loopback = Ipv4Addr::new(127, 0, 0, 1);
    let socket = SocketAddrV4::new(loopback, 0);
    let listener = TcpListener::bind(socket)?;
    let port = listener.local_addr()?.port();
    Ok(port)
}

pub struct PsqlServer {
    process: Child,
    base_dir: Option<TempDir>,
    pub port: u16
}

#[derive(Debug)]
pub enum PsqlServerError {
    CouldNotFindPostgresCommand,
    CouldNotFindInitDbCommand,
    CouldNotFindCreateDbCommand,
    CouldNotFindPgIsReadyCommand,
    InitDbFailed,
    CreateDbFailed,
    PostgresFailed,
    IoError(std::io::Error)
}

impl std::error::Error for PsqlServerError {
}

impl std::fmt::Display for PsqlServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            PsqlServerError::CouldNotFindPostgresCommand =>
                write!(f, "Could not find `postgres` command"),
            PsqlServerError::CouldNotFindInitDbCommand =>
                write!(f, "Could not find `initdb` command"),
            PsqlServerError::CouldNotFindCreateDbCommand =>
                write!(f, "Could not find `createdb` command"),
            PsqlServerError::CouldNotFindPgIsReadyCommand =>
                write!(f, "Could not find `pg_isready` command"),
            PsqlServerError::InitDbFailed =>
                write!(f, "initdb failed"),
            PsqlServerError::CreateDbFailed =>
                write!(f, "createdb failed"),
            PsqlServerError::PostgresFailed =>
                write!(f, "postgres failed"),
            PsqlServerError::IoError(error) =>
                write!(f, "{}", error)
        }
    }
}

impl PsqlServer {
    pub fn start() -> Result<PsqlServer, PsqlServerError> {
        let postgres = which("postgres")
            .map_err(|_| PsqlServerError::CouldNotFindPostgresCommand)?;
        let initdb = which("initdb")
            .map_err(|_| PsqlServerError::CouldNotFindInitDbCommand)?;
        let createdb = which("createdb")
            .map_err(|_| PsqlServerError::CouldNotFindCreateDbCommand)?;
        let pg_isready = which("pg_isready")
            .map_err(|_| PsqlServerError::CouldNotFindPgIsReadyCommand)?;

        let base_dir = TempDir::new("postgresql")
            .map_err(|e| PsqlServerError::IoError(e))?;
        let base_path = base_dir.path();
        let data_path = base_path.join("data").to_str()
            .unwrap().to_owned();
        let tmp_path = base_path.join("tmp").to_str()
            .unwrap().to_owned();
        fs::create_dir(&data_path)
            .map_err(|e| PsqlServerError::IoError(e))?;
        fs::create_dir(&tmp_path)
            .map_err(|e| PsqlServerError::IoError(e))?;

        let initdb_out = Command::new(&initdb)
            .args(&["-D", &data_path, "--lc-messages=C",
                    "-U", "postgres", "-A", "trust"])
            .output()
            .expect(&format!("failed to execute {}", initdb));

        if !initdb_out.status.success() {
            return Err(PsqlServerError::InitDbFailed);
        }

        let port = get_unused_port()
            .map_err(|e| PsqlServerError::IoError(e))?;

        let mut process = Command::new(postgres)
            .args(&["-p", &format!("{}", port),
                    "-D", &data_path,
                    "-k", &tmp_path,
                    "-h", "127.0.0.1",
                    "-F",
                    "-c", "logging_collector=off"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to execute psql");

        loop {
            if let Some(_exit_code) = process.try_wait()
                .map_err(|e| PsqlServerError::IoError(e))? {
                    return Err(PsqlServerError::PostgresFailed);
                }
            let isready_out = Command::new(&pg_isready)
                .args(&["-p", &format!("{}", port),
                        "-h", "127.0.0.1",
                        "-U", "postgres"])
                .output()
                .expect("failed to execute pg_isready");

            if isready_out.status.success() {
                break;
            } else {
                thread::sleep(Duration::from_millis(500))
            }
        }

        let createdb_out = Command::new(createdb)
            .args(&["-p", &format!("{}", port),
                    "-h", "127.0.0.1",
                    "-U", "postgres",
                    "test"])
            .output()
            .expect("failed to execute createdb");

        if !createdb_out.status.success() {
            return Err(PsqlServerError::CreateDbFailed);
        }

        Ok(PsqlServer {
            process,
            base_dir: Some(base_dir),
            port
        })
    }
}

impl fmt::Debug for PsqlServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PsqlServer {{ port: {}, base_dir: {} }}",
               self.port,
               self.base_dir.as_ref().unwrap().path().display())
    }
}

impl Drop for PsqlServer {
    fn drop(&mut self) {
        self.process.kill()
            .expect("failed to kill postgres");
        self.process.wait().expect("....");
        self.base_dir.take().unwrap().close().expect("failed to delete temp dir");
    }
}
