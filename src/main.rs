use reqwest::Client;
use tokio::sync::Mutex;

#[derive(serde::Serialize)]
struct Link {
    link: String,
}

#[derive(Debug)]
struct Offline;
#[derive(PartialEq, Debug)]
enum ResultOrOffline<T, E> {
    Ok(T),
    Err(E),
    Offline,
}
impl<T, E> ResultOrOffline<T, E> {
    fn is_err(&self) -> bool {
        matches!(*self, ResultOrOffline::Err(_))
    }
    fn is_offline(&self) -> bool {
        matches!(*self, ResultOrOffline::Offline)
    }
    fn unwrap(self) -> T {
        if let ResultOrOffline::Ok(s) = self {
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
) -> Result<(String, String), Offline> {
    let params = Link {
        link: url.to_string(),
    };
    let client = client.lock().await;
    let result = client.post(url!("api/add")).form(&params).send().await;
    if result.is_err() {
        return Err(Offline);
    }
    let result = result.unwrap();
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

    if result.is_err() {
        return ResultOrOffline::Offline;
    }

    let result = result.unwrap();
    if result.status() == reqwest::StatusCode::CONFLICT {
        return ResultOrOffline::Err(StridNotUnique);
    }

    let text = result.text().await.unwrap();
    let mut text = text.split(' ');
    ResultOrOffline::Ok((
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

    if result.is_err() {
        return ResultOrOffline::Offline;
    }

    let result = result.unwrap();
    if result.status() == reqwest::StatusCode::NOT_FOUND {
        ResultOrOffline::Err(NotFound)
    } else {
        let text = result.text().await.unwrap();
        let mut text = text.split(' ');
        ResultOrOffline::Ok((
            text.next()
                .expect("Server error: Expected CLICKS")
                .to_string(),
            text.next().expect("Server error: Expected URL").to_string(),
        ))
    }
}

#[tokio::main]
async fn main() {
    let subcommand = std::env::args().nth(1);
    if subcommand.is_none() {
        let command_name: String = std::env::args().next().unwrap();
        println!("[!] No arguments given!");
        println!("<?> Help:");
        println!(
            "The URLs are written in the `<url>(+<strid>)` format. \
             The `+` is a separator."
        );
        println!("{command_name} <urls>       | add every url listed");
        println!("{command_name} stats <urls> | check stats of every url");
        return;
    }
    let client = Mutex::new(Client::new());
    if subcommand.unwrap().to_lowercase() == "stats" {
        for arg in std::env::args().skip(2) {
            let stats = stats(&client, &arg).await;
            if stats.is_err() {
                println!("[!] {HOST}/{arg} not found");
            } else if stats.is_offline() {
                println!("[!] Offline or {HOST} unreachable");
                break;
            } else {
                let (clicks, url) = stats.unwrap();
                println!("{HOST}/{arg}:\n  - {}\n  - {} clicks", url, clicks)
            }
        }
        return;
    }

    for arg in std::env::args().skip(1) {
        let mut splitted_arg = arg.split('+');
        let link = splitted_arg.next().unwrap();
        let strid = splitted_arg.next();

        if strid.is_none() {
            let response = add(&client, link).await;
            if response.is_err() {
                println!("[!] Offline or {HOST} unreachable");
                break;
            }
            let (numid, strid) = response.unwrap();
            println!("{link}:\n  - {HOST}/{strid}\n  - {HOST}/.{numid}");
        } else {
            let strid = strid.unwrap();
            if !strid
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                println!(
                    "[!] String ID `{strid}` invalid. \
                          A String ID must only contain:"
                );
                println!("  - latin letters (A-Z and a-z");
                println!("  - minuses (-)");
                println!("  - underscores (_)");
                println!("  - numbers (0-9)");
                continue;
            }

            let response = add_with_strid(&client, link, strid).await;
            if response.is_err() {
                println!("[!] String ID `{}` already used", strid);
            } else if response.is_offline() {
                println!("[!] Offline or {HOST} unreachable");
            } else {
                let response = response.unwrap();
                let (numid, strid) = response;
                println!("{link}:\n  - {HOST}/{strid}\n  - {HOST}/.{numid}");
            }
        }
    }
}
