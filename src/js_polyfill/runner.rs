use std::process::{Command, Stdio};
use std::time::Duration;

use tracing::warn;

/// Execute a JS expression and return the stdout output.
/// Injects base URL, keyword, and polyfill.
pub fn execute_js(base_url: &str, keyword: &str, js_code: &str) -> Option<String> {
    let polyfill = include_str!("polyfill.js");

    // Build the combined script
    let script = format!(
        r#"
globalThis.__BASE_URL__ = {:?};
globalThis.__KEYWORD__ = {:?};
globalThis.__PAGE__ = 1;
globalThis.__SOURCE_KEY__ = '';
globalThis.__LOGIN_HEADER__ = '';
globalThis.__SOURCE_COMMENT__ = '';
globalThis.__lastResponse = '';
globalThis.__lastUrl = '';
globalThis.__lastMethod = 'GET';
globalThis.__lastHeaders = {{}};
globalThis.__lastBody = '';

{}

try {{
    {};

    let url = typeof url !== 'undefined' ? url :
              (java._store['url'] || '');

    if (url) {{
        console.log(JSON.stringify({{type:'result',url:String(url),method:'GET',headers:{{}}}}));
    }} else {{
        console.log(JSON.stringify({{type:'result',url:'',method:'GET',headers:{{}}}}));
    }}
}} catch(e) {{
    console.error('[JS_ERROR]', String(e));
    console.log(JSON.stringify({{type:'error',message:String(e)}}));
}}
"#,
        base_url, keyword, polyfill, js_code
    );

    let mut child = match Command::new("node")
        .arg("-e")
        .arg(&script)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to spawn node: {}", e);
            return None;
        }
    };

    // Wait with timeout
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(10);

    loop {
        if start.elapsed() > timeout {
            if let Err(e) = child.kill() {
                warn!("Failed to kill timed-out node process: {}", e);
            } else {
                // Wait briefly for the process to exit after kill
                let _ = child.wait();
            }
            warn!("Node subprocess timed out after 10s");
            return None;
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                warn!("Node subprocess error: {}", e);
                return None;
            }
        }
    }

    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to read node output: {}", e);
            return None;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("Node exited with error: {}", stderr.trim());
        return None;
    }

    // Parse stdout — look for the last JSON result line
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed)
            && val.get("type").and_then(|v| v.as_str()) == Some("result") {
                return val.get("url").and_then(|v| v.as_str()).map(|s| s.to_string());
            }
    }

    None
}
