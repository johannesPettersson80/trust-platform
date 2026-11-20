//! A stand-in for an electricity price source: it answers `GET /price` on 127.0.0.1:47801 with
//! `{"price": <price>, "unit": "EUR/MWh"}`, at the price you give it and then at each price you
//! type. Run it in the project folder with
//! `cargo run --manifest-path price-service/Cargo.toml --example stand_in_source -- 38.2`.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, PoisonError};

/// Where the price service looks for its source unless `PRICE_SOURCE` names another.
const ADDRESS: &str = "127.0.0.1:47801";

fn main() -> std::io::Result<()> {
    let first = match std::env::args().nth(1) {
        Some(text) => text.parse::<f64>().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{text} is no price"),
            )
        })?,
        None => 38.2,
    };
    let price = Arc::new(Mutex::new(first));
    let listener = TcpListener::bind(ADDRESS)?;
    println!(
        "Serving {first} EUR/MWh at http://{ADDRESS}/price. Type another price and press Enter."
    );
    let served = Arc::clone(&price);
    let server = std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let now = *served.lock().unwrap_or_else(PoisonError::into_inner);
            answer(stream, now);
        }
    });
    for line in std::io::stdin().lines() {
        let line = line?;
        match line.trim().parse::<f64>() {
            Ok(new) => {
                *price.lock().unwrap_or_else(PoisonError::into_inner) = new;
                println!("Now {new} EUR/MWh.");
            }
            Err(_) => println!("{} is no price; type a number such as 120.", line.trim()),
        }
    }
    // Without a terminal to read prices from, it keeps serving the last one.
    let _ = server.join();
    Ok(())
}

/// Reads the request to its end, then answers `GET /price` with `price` and anything else with 404.
fn answer(stream: TcpStream, price: f64) {
    let mut reader = BufReader::new(&stream);
    let mut request = String::new();
    if reader.read_line(&mut request).is_err() {
        return;
    }
    let mut header = String::new();
    while reader.read_line(&mut header).is_ok_and(|read| read > 2) {
        header.clear();
    }
    let (status, body) = if request.starts_with("GET /price ") {
        (
            "200 OK",
            format!("{{\"price\": {price}, \"unit\": \"EUR/MWh\"}}"),
        )
    } else {
        ("404 Not Found", "{\"error\": \"not found\"}".to_owned())
    };
    let mut stream = &stream;
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}
