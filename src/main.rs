use tokio::net::UnixStream;
use tokio::io::{self, AsyncWriteExt, AsyncReadExt};

#[tokio::main]
async fn main() -> io::Result<()> {

    let args = std::env::args().collect::<Vec<String>>();
    
    if args.len() < 2 {
        print!("{}", serde_json::json!({
            "code": 400,
            "message": "Usage: wei-docker-linux <image_name>"
        }));
        return Ok(());
    }

    let mut stream = match UnixStream::connect("/var/run/docker.sock").await {
        Ok(stream) => stream,
        Err(_) => {
            print!("{}", serde_json::json!({
                "code": 500,
                "message": "Failed to connect docker.sock"
            }));
            return Ok(());
        }
    };

    let image_name = &args[1];

    let image_name = if image_name.contains(":") {
        format!("{}", image_name)
    } else {
        format!("{}:latest", image_name)
    };

    let request = format!(
        "POST /v1.43/images/create?fromImage={} HTTP/1.1\r\nHost: localhost\r\n\r\n", 
        image_name
    );

    match stream.write_all(request.as_bytes()).await {
        Ok(_) => {},
        Err(_) => {
            print!("{}", serde_json::json!({
                "code": 500,
                "message": "Failed to write request"
            }));
            return Ok(());
        }
    };

    let mut buffer = [0; 1024]; // 创建一个缓冲区

    // 持续读取并输出数据
    loop {
        let n = match stream.read(&mut buffer).await {
            Ok(n) => n,
            Err(_) => continue
        };
        if n == 0 {
            break; // 如果没有更多数据可读，则退出循环
        }
        // timeout 接收到timout 就退出

        let data = String::from_utf8_lossy(&buffer[..n]);

        if data.contains("i/o timeout") {
            print!("{}", serde_json::json!({
                "code": 500,
                "message": "Timeout"
            }));
            break;
        }

        if data.contains("Image is up to date for") ||
           data.contains("Downloaded newer image for ") {
            break;
        }

        print!("{}", data);
    }

    print!("{}", serde_json::json!({
        "code": 200,
        "message": "Success"
    }));

    Ok(())
}
