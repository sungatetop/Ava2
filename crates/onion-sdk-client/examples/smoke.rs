//! 冒烟 example：驱动 release onion-sdk-server，stderr 透传观察 server 日志。
//! 运行：cargo run -p onion-sdk-client --example smoke --release

use onion_sdk_client::OnionSdkClient;

fn main() {
    let server = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/release/onion-sdk-server.exe".to_string());
    println!("[smoke] spawn server: {server}");
    let mut client = match OnionSdkClient::spawn(&server) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[smoke] spawn 失败: {e}");
            std::process::exit(1);
        }
    };
    println!("[smoke] session_id = {}", client.session_id());

    match client.prompt("请只回复两个字：收到", "followup") {
        Ok(id) => println!("[smoke] prompt message_id = {id}"),
        Err(e) => {
            eprintln!("[smoke] prompt 失败: {e}");
            std::process::exit(1);
        }
    }

    match client.wait_idle(Some(60_000)) {
        Ok(idle) => println!("[smoke] idle = {idle}"),
        Err(e) => {
            eprintln!("[smoke] wait 失败: {e}");
            std::process::exit(1);
        }
    }

    println!("[smoke] calling shutdown ...");
    match client.shutdown() {
        Ok(()) => println!("[smoke] shutdown ok"),
        Err(e) => eprintln!("[smoke] shutdown 失败: {e}"),
    }
}
