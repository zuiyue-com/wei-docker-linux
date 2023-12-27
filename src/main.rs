#[macro_use]
extern crate wei_log;

use tokio::net::UnixStream;
use tokio::io::{self, AsyncWriteExt, AsyncReadExt};

use serde_json::{Value, Map};

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use std::fs::{self};
use std::io::Write;

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let mut accumulated_json = Map::new();
    
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

    let image_name_encoded = utf8_percent_encode(&image_name, NON_ALPHANUMERIC).to_string();
    let file_path = format!("/root/.wei/docker/{}.json", image_name_encoded);
    
    if args.len() > 2 {
        report(args[2].clone(), file_path.clone()).await;
    }

    match fs::create_dir_all("/root/.wei/docker/") {
        Ok(_) => {},
        Err(_) => {
            print!("{}", serde_json::json!({
                "code": 500,
                "message": "Failed to create directory"
            }));
            return Ok(());
        }
    }
    
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

        let data = String::from_utf8_lossy(&buffer[..n]);

        info!("{}", data);

        let parsed_json = parse_chunked_response(&data);
        for json in parsed_json {
            match json {
                Ok(obj) => merge_json_by_id(&mut accumulated_json, obj),
                Err(e) => println!("Failed to parse JSON: {}", e),
            }
        }

        if let Ok(formatted_json) = serde_json::to_string_pretty(&accumulated_json) {
            
            let mut file = match std::fs::File::create(file_path.clone()) {
                Ok(file) => file,
                Err(_) => {
                    print!("{}", serde_json::json!({
                        "code": 500,
                        "message": "Failed to create file"
                    }));
                    return Ok(());
                }
            };
            match file.write_all(formatted_json.as_bytes()) {
                Ok(_) => {},
                Err(_) => {
                    print!("{}", serde_json::json!({
                        "code": 500,
                        "message": "Failed to write file"
                    }));
                    return Ok(());
                }
            };
        } 
        
        if data.contains("i/o timeout") {
            print!("{}", serde_json::json!({
                "code": 500,
                "message": "Timeout"
            }));
            return Ok(());
        }

        if data.contains("Image is up to date for") ||
           data.contains("Downloaded newer image for") {
            break;
        }

    }

    print!("{}", serde_json::json!({
        "code": 200,
        "message": "Success"
    }));

    Ok(())
}

async fn report(url: String, file_path: String) {
    tokio::spawn(async move {
        loop {
            let content = match std::fs::read_to_string(file_path.clone()) {
                Ok(content) => content,
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    continue;
                }
            };

            let client = reqwest::Client::new();
            match client.post(url.clone())
                .header("accept", "application/json")
                .header("Content-Type", "application/json")
                .body(content.clone())
                .send().await {
                    Ok(_) => {},
                    Err(_) => {},
                };

            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });
}

// 合并json
fn merge_json_by_id(accumulated: &mut Map<String, Value>, new_json: Value) {
    if let Some(id) = new_json["id"].as_str() {
        let entry = accumulated.entry(id.to_string()).or_insert_with(|| serde_json::json!({}));
        merge_entry(entry, new_json);
    }
}

// 合并json
fn merge_entry(entry: &mut Value, new_json: Value) {
    if let (Some(entry_obj), Some(new_obj)) = (entry.as_object_mut(), new_json.as_object()) {
        for (key, value) in new_obj {
            entry_obj.insert(key.to_string(), value.clone());
        }
    }
}

// 只分析chunked部分
fn parse_chunked_response(data: &str) -> Vec<Result<Value, serde_json::Error>> {
    let mut results = Vec::new();

    // 解析chunked部分
    let mut lines = data.lines();
    while let Some(line) = lines.next() {
        if let Ok(size) = usize::from_str_radix(line, 16) {
            if size == 0 { break; } // 如果大小为0，则表示chunked传输结束

            if let Some(json_str) = lines.next() {
                let result = serde_json::from_str(json_str);
                results.push(result);
            }
        }
    }

    results
}
