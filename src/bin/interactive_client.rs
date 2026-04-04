use rmcp::{ServiceExt, model::CallToolRequestParams, transport::StreamableHttpClientTransport};
use serde_json::Value;
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

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let url_arg = std::env::args().nth(1);
    let resolved_url = resolve_url_arg(url_arg)?;
    let url = normalize_client_url(resolved_url)?;

    let transport = StreamableHttpClientTransport::from_uri(url);
    let client = ().serve(transport).await?;

    println!("interactive MCP client ready");
    println!("use: <tool_name> <json-object-args>");
    println!("example: openmcpgdb_debugger_state {{}}");
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

                let pretty = serde_json::to_string_pretty(&display_value)
                    .unwrap_or_else(|_| display_value.to_string());
                if response.is_error == Some(true) {
                    println!("error_response:");
                } else {
                    println!("response:");
                }
                println!("{pretty}");
            }
            Err(err) => {
                eprintln!("tool call failed: {err}");
            }
        }
    }

    client.cancel().await?;
    Ok(())
}
