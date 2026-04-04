use rmcp::{ServiceExt, model::CallToolRequestParams, transport::StreamableHttpClientTransport};
use std::io::{self, Write};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://127.0.0.1:9443/mcp".to_string());

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
                for content in response.content {
                    if let Some(text) = content.raw.as_text() {
                        println!("{}", text.text);
                    } else {
                        println!("{:?}", content.raw);
                    }
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
