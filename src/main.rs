use reqwest::Client;
use tokio::sync::Mutex;

#[derive(serde::Serialize)]
struct Link {
    link: String,
}

//errors
#[derive(Debug)]
struct OfflineError;
#[derive(Debug)]
struct TooShort;
#[derive(PartialEq, Debug)]
enum ResultOrOffline<T, E> {
    Ok(T),
    Err(E),
    OfflineError,
}

use ResultOrOffline::Err;
use ResultOrOffline::OfflineError as Offline;
use ResultOrOffline::Ok;

impl<T, E> ResultOrOffline<T, E> {
    fn is_err(&self) -> bool {
        matches!(*self, Err(_))
    }
    fn is_offline(&self) -> bool {
        matches!(*self, Offline)
    }
    fn unwrap(self) -> T {
        if let Ok(s) = self {
            s
        } else {
            panic!("ResultOrOffline not OK")
        }
    }
}

const HOST: &str = "http://localhost:8080";
macro_rules! url {
    ($path: expr, $last_elem: expr) => {
        &format!("{HOST}/{}/{}", $path, $last_elem)
    };
    ($path: expr) => {
        &format!("{HOST}/{}", $path)
    };
}

async fn add(
    client: &Mutex<Client>,
    url: &str,
) -> ResultOrOffline<(String, String), TooShort> {
    let params = Link {
        link: url.to_string(),
    };

    let result = {
        let client = client.lock().await;
        client.post(url!("api/add")).form(&params).send().await
    };

    if result.is_err() {
        return Offline;
    }
    let result = result.unwrap();
    if result.status() == reqwest::StatusCode::URI_TOO_LONG {
        return Err(TooShort);
    }

    let text = result.text().await.unwrap();
    let mut text = text.split(' ');
    Ok((
        text.next()
            .expect("Server error: Expected NUMID")
            .to_string(),
        text.next()
            .expect("Server error: Expected STRID")
            .to_string(),
    ))
}

#[derive(Debug, PartialEq)]
struct StridNotUnique;

async fn add_with_strid(
    client: &Mutex<Client>,
    url: &str,
    strid: &str,
) -> ResultOrOffline<(String, String), StridNotUnique> {
    let params = Link {
        link: url.to_string(),
    };

    let client = client.lock().await;
    let result = client
        .post(url!("api/add", strid))
        .form(&params)
        .send()
        .await;
    drop(client);

    if result.is_err() {
        return Offline;
    }

    let result = result.unwrap();
    if result.status() == reqwest::StatusCode::CONFLICT {
        return Err(StridNotUnique);
    }

    let text = result.text().await.unwrap();
    let mut text = text.split(' ');
    Ok((
        text.next()
            .expect("Server error: Expected NUMID")
            .to_string(),
        text.next()
            .expect("Server error: Expected STRID")
            .to_string(),
    ))
}

#[derive(PartialEq, Debug)]
struct NotFound;

async fn stats(
    client: &Mutex<Client>,
    strid: &str,
) -> ResultOrOffline<(String, String), NotFound> {
    let client = client.lock().await;
    let result = client.get(url!("api/stats", strid)).send().await;
    drop(client);

    if result.is_err() {
        return Offline;
    }

    let result = result.unwrap();
    if result.status() == reqwest::StatusCode::NOT_FOUND {
        Err(NotFound)
    } else {
        let text = result.text().await.unwrap();
        let mut text = text.split(' ');
        Ok((
            text.next()
                .expect("Server error: Expected CLICKS")
                .to_string(),
            text.next().expect("Server error: Expected URL").to_string(),
        ))
    }
}

fn is_valid(strid: &str) -> bool {
    strid
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[tokio::main]
async fn main() {
    let subcommand = std::env::args().nth(1);
    if subcommand.is_none() {
        let command_name: String = std::env::args().next().unwrap();
        eprintln!("[!] No arguments given!");
        eprintln!("<?> Help:");
        eprintln!(
            "The URLs are written in the `<url>(+<strid>)` format. \
             The `+` is a separator."
        );
        eprintln!("{command_name} <urls>       | add every url listed");
        eprintln!("{command_name} stats <urls> | check stats of every url");
        return;
    }

    let client = Mutex::new(Client::new());

    if subcommand.unwrap().to_lowercase() == "stats" {
        for arg in std::env::args().skip(2) {
            let stats = stats(&client, &arg).await;
            if stats.is_err() {
                eprintln!("[!] {HOST}/{arg} not found");
            } else if stats.is_offline() {
                eprintln!("[!] Offline or {HOST} unreachable");
                break;
            } else {
                let (clicks, url) = stats.unwrap();
                println!("{HOST}/{arg}:\n  - {}\n  - {} clicks", url, clicks)
            }
        }
    } else {
        for arg in std::env::args().skip(1) {
            let mut splitted_arg = arg.split('+');
            let link = splitted_arg.next().unwrap();
            let strid = splitted_arg.next();

            if strid.is_none() {
                let response = add(&client, link).await;
                if response.is_offline() {
                    eprintln!("[!] Offline or {HOST} unreachable");
                    break;
                } else if response.is_err() {
                    eprintln!("[!] `{link}` is too short to be shortened");
                    break;
                }
                let (numid, strid) = response.unwrap();
                println!("{link}:\n  - {HOST}/{strid}\n  - {HOST}/.{numid}");
            } else {
                let strid = strid.unwrap();
                if !is_valid(strid) {
                    eprintln!(
                        "[!] String ID `{strid}` invalid. \
                         A String ID must only contain:"
                    );
                    eprintln!("  - latin letters (A-Z and a-z");
                    eprintln!("  - minuses (-)");
                    eprintln!("  - underscores (_)");
                    eprintln!("  - numbers (0-9)");
                    continue;
                }

                let response = add_with_strid(&client, link, strid).await;
                if response.is_err() {
                    eprintln!("[!] String ID `{}` already used", strid);
                } else if response.is_offline() {
                    eprintln!("[!] Offline or {HOST} unreachable");
                } else {
                    let (numid, strid) = response.unwrap();
                    println!(
                        "{link}:\n  - {HOST}/{strid}\n  - {HOST}/.{numid}"
                    );
                }
            }
        }
    }
}
