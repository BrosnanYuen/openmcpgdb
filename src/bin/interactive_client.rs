use rmcp::{ServiceExt, model::CallToolRequestParams, transport::StreamableHttpClientTransport};
use serde_json::{Map, Value};
use std::{
    io::{self, Write},
    path::Path,
};
use url::Url;

fn resolve_url_arg(arg: Option<String>) -> anyhow::Result<String> {
    match arg {
        Some(value) => {
            let path = Path::new(&value);
            if path.exists() {
                let config = openmcpgdb::ServerConfig::from_file(path)?;
                return Ok(config.mcp_server_url);
            }
            Ok(value)
        }
        None => {
            let default_path = Path::new("config.json");
            if default_path.exists() {
                let config = openmcpgdb::ServerConfig::from_file(default_path)?;
                return Ok(config.mcp_server_url);
            }
            Ok("https://localhost:9443".to_string())
        }
    }
}

fn normalize_client_url(input: String) -> anyhow::Result<String> {
    let parsed = Url::parse(&input)?;
    if parsed.scheme() == "stdio" {
        anyhow::bail!("interactive_client supports HTTP(S) only; received stdio URL");
    }

    // The server currently binds plain HTTP sockets; convert https URL to http for local testing.
    if parsed.scheme() == "https" {
        let mut converted = parsed;
        converted
            .set_scheme("http")
            .map_err(|_| anyhow::anyhow!("failed to convert https URL to http"))?;
        return Ok(converted.to_string());
    }

    Ok(input)
}

fn sort_json_for_display(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json_for_display).collect()),
        Value::Object(object) => {
            let mut entries: Vec<(String, Value)> = object
                .into_iter()
                .map(|(key, val)| (key, sort_json_for_display(val)))
                .collect();

            entries.sort_by(|(left_key, _), (right_key, _)| {
                let left_num = left_key.parse::<u64>().ok();
                let right_num = right_key.parse::<u64>().ok();
                match (left_num, right_num) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => left_key.cmp(right_key),
                }
            });

            let mut sorted = Map::new();
            for (key, val) in entries {
                sorted.insert(key, val);
            }
            Value::Object(sorted)
        }
        _ => value,
    }
}

fn format_response_for_display(value: Value) -> (Value, Option<String>) {
    if let Value::Object(mut object) = value {
        if let Some(Value::String(code)) = object.get("current_code") {
            if code.contains('\n') {
                let code_block = code.clone();
                object.insert(
                    "current_code".to_string(),
                    Value::String("<multiline output below>".to_string()),
                );
                return (Value::Object(object), Some(code_block));
            }
        }
        return (Value::Object(object), None);
    }
    (value, None)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let url_arg = std::env::args().nth(1);
    let resolved_url = resolve_url_arg(url_arg)?;
    let url = normalize_client_url(resolved_url)?;

    let transport = StreamableHttpClientTransport::from_uri(url);
    let client = ().serve(transport).await?;

    println!("interactive MCP client ready");
    println!("use: <tool_name> <json-object-args>");
    println!("example: gdb_debugger_state {{}}");
    println!("type 'quit' to exit");

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut line = String::new();
        let read = io::stdin().read_line(&mut line)?;
        if read == 0 {
            break;
        }

        let line = line.trim();
        if line == "quit" {
            break;
        }
        if line.is_empty() {
            continue;
        }

        let mut parts = line.splitn(2, ' ');
        let Some(tool_name) = parts.next() else {
            continue;
        };
        let tool_name = tool_name.to_string();
        let args = parts.next().unwrap_or("{}");

        let args_json: serde_json::Value = match serde_json::from_str(args) {
            Ok(value) => value,
            Err(err) => {
                eprintln!("invalid json args: {err}");
                continue;
            }
        };

        let args_obj = args_json.as_object().cloned().unwrap_or_default();
        let result = client
            .call_tool(CallToolRequestParams::new(tool_name).with_arguments(args_obj))
            .await;

        match result {
            Ok(response) => {
                let display_value = if let Some(structured) = response.structured_content {
                    structured
                } else if let Some(first) = response.content.first() {
                    if let Some(text) = first.raw.as_text() {
                        serde_json::from_str::<Value>(&text.text)
                            .unwrap_or_else(|_| Value::String(text.text.to_string()))
                    } else {
                        Value::String(format!("{:?}", first.raw))
                    }
                } else {
                    Value::Null
                };

                let sorted_display_value = sort_json_for_display(display_value);
                let (display_json, current_code_block) =
                    format_response_for_display(sorted_display_value);
                let pretty = serde_json::to_string_pretty(&display_json)
                    .unwrap_or_else(|_| display_json.to_string());
                if response.is_error == Some(true) {
                    println!("error_response:");
                } else {
                    println!("response:");
                }
                println!("{pretty}");
                if let Some(code) = current_code_block {
                    println!("current_code:");
                    println!("{code}");
                }
            }
            Err(err) => {
                eprintln!("tool call failed: {err}");
            }
        }
    }

    client.cancel().await?;
    Ok(())
}
