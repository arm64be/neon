use std::{
    collections::HashMap,
    env, fs,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::Arc,
    thread,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const DEFAULT_ADDR: &str = "127.0.0.1:3000";
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const DEFAULT_MODEL: &str = "openai/gpt-oss-20b";
const OPENROUTER_MAX_RETRIES: usize = 3;

#[derive(Clone)]
struct Asset {
    content_type: &'static str,
    bytes: Arc<Vec<u8>>,
}

#[derive(Clone, Default)]
struct Site {
    static_assets: HashMap<String, Asset>,
    docs_pages: HashMap<String, Asset>,
}

impl Site {
    fn load() -> io::Result<Self> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let static_dir = manifest_dir.join("static");
        let docs_dir = manifest_dir.join("..").join("docs");

        let mut site = Self::default();
        site.load_static_dir(&static_dir)?;
        site.load_docs_dir(&docs_dir)?;
        Ok(site)
    }

    fn load_static_dir(&mut self, dir: &Path) -> io::Result<()> {
        for path in walk_files(dir)? {
            let rel = path.strip_prefix(dir).map_err(io::Error::other)?;
            let url_path = to_lower_url_path(rel);
            let bytes = fs::read(&path)?;
            let content_type = content_type_for_path(&path);
            let asset = Asset {
                content_type,
                bytes: Arc::new(bytes),
            };

            if rel == Path::new("index.html") {
                self.static_assets.insert("/".to_string(), asset.clone());
            }

            if url_path.ends_with("/index.html") {
                let dir_path = url_path.trim_end_matches("index.html");
                self.static_assets
                    .insert(dir_path.to_string(), asset.clone());
            }

            self.static_assets.insert(url_path.clone(), asset.clone());
            if rel == Path::new("index.html") {
                self.static_assets
                    .insert("/index.html".to_string(), asset.clone());
            }
        }

        Ok(())
    }

    fn load_docs_dir(&mut self, dir: &Path) -> io::Result<()> {
        for path in walk_files(dir)? {
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }

            let rel = path.strip_prefix(dir).map_err(io::Error::other)?;
            let rendered = render_markdown_page(&path, &fs::read_to_string(&path)?)?;
            let asset = Asset {
                content_type: "text/html; charset=utf-8",
                bytes: Arc::new(rendered.into_bytes()),
            };

            let route = format!("/docs/{}", to_lower_url_path(rel));
            self.docs_pages.insert(route.clone(), asset.clone());

            if rel
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("INDEX.md"))
            {
                let dir_route = match rel.parent() {
                    Some(parent) if !parent.as_os_str().is_empty() => {
                        format!("/docs/{}/", to_lower_url_path(parent))
                    }
                    _ => "/docs/".to_string(),
                };
                self.docs_pages.insert(dir_route, asset.clone());
                if rel == Path::new("INDEX.md") {
                    self.docs_pages.insert("/docs/".to_string(), asset.clone());
                    self.docs_pages
                        .insert("/docs/index.md".to_string(), asset.clone());
                }
            }
        }

        Ok(())
    }

    fn resolve(&self, path: &str) -> Option<&Asset> {
        let normalized = path.to_ascii_lowercase();

        if let Some(asset) = self.static_assets.get(&normalized) {
            return Some(asset);
        }

        if let Some(asset) = self.docs_pages.get(&normalized) {
            return Some(asset);
        }

        if normalized == "/docs" {
            return self.docs_pages.get("/docs/");
        }

        if normalized == "/" {
            return self.static_assets.get("/");
        }

        if let Some(stripped) = normalized.strip_prefix("/docs/") {
            let stripped = stripped.trim_end_matches('/');
            if stripped.is_empty() {
                return self.docs_pages.get("/docs/");
            }

            let mut candidates = Vec::new();
            candidates.push(format!("/docs/{}", stripped));
            candidates.push(format!("/docs/{}.md", stripped));
            candidates.push(format!("/docs/{}/index.md", stripped));

            for candidate in candidates {
                if let Some(asset) = self.docs_pages.get(&candidate) {
                    return Some(asset);
                }
            }
        }

        if let Some(stripped) = normalized.strip_prefix('/') {
            let stripped = stripped.trim_end_matches('/');
            let mut candidates = Vec::new();
            candidates.push(format!("/{}", stripped));
            if stripped.is_empty() {
                candidates.push("/index.html".to_string());
            } else if !stripped.contains('.') {
                candidates.push(format!("/{}.html", stripped));
                candidates.push(format!("/{}/index.html", stripped));
            }

            for candidate in candidates {
                if let Some(asset) = self.static_assets.get(&candidate) {
                    return Some(asset);
                }
            }
        }

        None
    }
}

fn main() -> io::Result<()> {
    let site = Arc::new(Site::load()?);
    let addr = env::var("NEON_BLUE_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let listener = TcpListener::bind(&addr)?;
    eprintln!("neon_blue listening on http://{addr}");

    for connection in listener.incoming() {
        let site = Arc::clone(&site);
        thread::spawn(move || {
            if let Ok(stream) = connection {
                let _ = handle_connection(stream, &site);
            }
        });
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream, site: &Site) -> io::Result<()> {
    let mut buffer = [0_u8; 8192];
    let size = stream.read(&mut buffer)?;
    if size == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..size]);
    let mut lines = request.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");

    if method == "POST" && path == "/v1/onboarding/providers" {
        let body = read_request_body(&mut stream, &request, &buffer[..size])?;
        let response = onboarding_providers(&body);
        write_json_response(&mut stream, response.status, response.reason, response.body)?;
        return Ok(());
    }

    if method != "GET" && method != "HEAD" {
        write_response(
            &mut stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            b"method not allowed",
        )?;
        return Ok(());
    }

    if let Some(asset) = site.resolve(path) {
        let body = asset.bytes.as_slice();
        let body = if method == "HEAD" { &[][..] } else { body };
        write_response(&mut stream, 200, "OK", asset.content_type, body)?;
        return Ok(());
    }

    write_response(
        &mut stream,
        404,
        "Not Found",
        "text/plain; charset=utf-8",
        b"not found",
    )?;
    Ok(())
}

#[derive(Deserialize)]
struct OnboardingProvidersRequest {
    stage: Option<String>,
    answer: String,
    answers: Option<Value>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ProviderSchema {
    id: String,
    name: String,
    base_url: String,
    #[serde(rename = "type")]
    provider_type: String,
    authentication: String,
}

#[derive(Deserialize)]
struct ModelProvidersResponse {
    providers: Vec<ProviderSchema>,
}

#[derive(Serialize)]
struct OnboardingProvidersResponse {
    providers: Vec<ProviderSchema>,
}

struct JsonResponse {
    status: u16,
    reason: &'static str,
    body: Value,
}

fn onboarding_providers(body: &[u8]) -> JsonResponse {
    let request: OnboardingProvidersRequest = match serde_json::from_slice(body) {
        Ok(request) => request,
        Err(err) => {
            return json_error(400, "Bad Request", format!("invalid request JSON: {err}"));
        }
    };

    let _ = request.stage.as_deref();
    let _ = request.answers.as_ref();

    if request.answer.trim().is_empty() {
        return json_error(400, "Bad Request", "answer is required".to_string());
    }

    let openrouter_key = match env::var("OPENROUTER_API_KEY") {
        Ok(key) if !key.trim().is_empty() => key,
        _ => {
            return json_error(
                500,
                "Internal Server Error",
                "OPENROUTER_API_KEY is not set".to_string(),
            );
        }
    };

    match call_openrouter(&openrouter_key, &request.answer) {
        Ok(providers) => JsonResponse {
            status: 200,
            reason: "OK",
            body: json!(OnboardingProvidersResponse { providers }),
        },
        Err(err) => json_error(502, "Bad Gateway", err),
    }
}

fn call_openrouter(api_key: &str, answer: &str) -> Result<Vec<ProviderSchema>, String> {
    let mut last_error = None;
    for attempt in 1..=OPENROUTER_MAX_RETRIES {
        match call_openrouter_once(api_key, answer, attempt) {
            Ok(providers) => return Ok(providers),
            Err(err) => last_error = Some(err),
        }
    }

    Err(format!(
        "provider model returned invalid output after {} attempts: {}",
        OPENROUTER_MAX_RETRIES,
        last_error.unwrap_or_else(|| "unknown error".to_string())
    ))
}

fn call_openrouter_once(
    api_key: &str,
    answer: &str,
    attempt: usize,
) -> Result<Vec<ProviderSchema>, String> {
    let model = env::var("NEON_ONBOARDING_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let mut system = onboarding_system_prompt();
    if attempt > 1 {
        system.push_str(
            "\nPrevious response was invalid. Return only valid JSON that matches the schema exactly.",
        );
    }

    let payload = json!({
        "model": model,
        "temperature": 0.0,
        "messages": [
            {
                "role": "system",
                "content": system
            },
            {
                "role": "user",
                "content": answer
            }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "provider_schemas",
                "strict": true,
                "schema": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "providers": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "id": { "type": "string" },
                                    "name": { "type": "string" },
                                    "base_url": { "type": "string" },
                                    "type": { "type": "string", "enum": ["openai", "anthropic"] },
                                    "authentication": { "type": "string", "enum": ["none", "api_key"] }
                                },
                                "required": ["id", "name", "base_url", "type", "authentication"]
                            }
                        }
                    },
                    "required": ["providers"]
                }
            }
        }
    });

    let response: Value = ureq::post(OPENROUTER_URL)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", "application/json")
        .set("HTTP-Referer", "https://github.com/arm64be/neon")
        .set("X-Title", "neon onboarding providers")
        .send_json(payload)
        .map_err(|err| format!("OpenRouter request failed: {err}"))?
        .into_json()
        .map_err(|err| format!("failed to decode OpenRouter response JSON: {err}"))?;

    let content = response
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|msg| msg.get("content"))
        .and_then(Value::as_str)
        .ok_or_else(|| "OpenRouter response missing choices[0].message.content".to_string())?;

    parse_model_providers_response(content)
}

fn parse_model_providers_response(content: &str) -> Result<Vec<ProviderSchema>, String> {
    if let Ok(parsed) = serde_json::from_str::<ModelProvidersResponse>(content) {
        validate_provider_schemas(&parsed.providers)?;
        return Ok(parsed.providers);
    }

    if let Ok(providers) = serde_json::from_str::<Vec<ProviderSchema>>(content) {
        validate_provider_schemas(&providers)?;
        return Ok(providers);
    }

    Err(serde_json::from_str::<ModelProvidersResponse>(content)
        .err()
        .map(|err| format!("model returned invalid JSON: {err}"))
        .unwrap_or_else(|| "model returned invalid JSON".to_string()))
}

fn validate_provider_schemas(providers: &[ProviderSchema]) -> Result<(), String> {
    if providers.is_empty() {
        return Err("provider list is empty".to_string());
    }

    for provider in providers {
        if !provider
            .id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch == '-')
        {
            return Err(format!(
                "provider id must match [a-z-]+, got {}",
                provider.id
            ));
        }
        if provider.provider_type != "openai" && provider.provider_type != "anthropic" {
            return Err(format!(
                "provider type must be openai or anthropic, got {}",
                provider.provider_type
            ));
        }
        if provider.authentication != "none" && provider.authentication != "api_key" {
            return Err(format!(
                "provider authentication must be none or api_key, got {}",
                provider.authentication
            ));
        }
        if provider.base_url.trim().is_empty() {
            return Err(format!("provider {} has an empty base_url", provider.id));
        }
    }

    Ok(())
}

fn onboarding_system_prompt() -> String {
    [
        "You are a provider parser for CLI onboarding.",
        "Return JSON only and follow the provided JSON Schema exactly.",
        "Parse user intent and return all providers they asked for.",
        "Return each provider as a full object with id, name, base_url, type, and authentication.",
        "If user mentions broad options (e.g. \"all common\"), include these common providers when relevant:",
        "- openrouter: OpenRouter, base_url=https://openrouter.ai/api/v1, type=openai, authentication=api_key",
        "- openai: OpenAI, base_url=https://api.openai.com/v1, type=openai, authentication=api_key",
        "- anthropic: Anthropic, base_url=https://api.anthropic.com/v1, type=anthropic, authentication=api_key",
        "- groq: Groq, base_url=https://api.groq.com/openai/v1, type=openai, authentication=api_key",
        "- xai: xAI, base_url=https://api.x.ai/v1, type=openai, authentication=api_key",
        "- deepseek: DeepSeek, base_url=https://api.deepseek.com/v1, type=openai, authentication=api_key",
        "- mistral: Mistral, base_url=https://api.mistral.ai/v1, type=openai, authentication=api_key",
        "- google-ai-studio: Google AI Studio (Gemini OpenAI compatibility), base_url=https://generativelanguage.googleapis.com/v1beta/openai, type=openai, authentication=api_key",
        "- azure-openai: Azure OpenAI, base_url=https://{resource}.openai.azure.com/openai/v1, type=openai, authentication=api_key",
        "- ollama-local: Ollama (local), base_url=http://localhost:11434/v1, type=openai, authentication=none",
        "For unknown providers, infer a best-effort schema using openai type unless clearly anthropic.",
        "Never include extra fields.",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_full_provider_objects() {
        let response = r#"{"providers":[{"id":"custom-openai","name":"Custom OpenAI","base_url":"https://example.com/v1","type":"openai","authentication":"api_key"}]}"#;
        let providers = parse_model_providers_response(response).unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "custom-openai");
        assert_eq!(providers[0].base_url, "https://example.com/v1");
    }

    #[test]
    fn accepts_bare_provider_arrays() {
        let response = r#"[{"id":"custom-openai","name":"Custom OpenAI","base_url":"https://example.com/v1","type":"openai","authentication":"api_key"}]"#;
        let providers = parse_model_providers_response(response).unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "custom-openai");
    }
}

fn read_request_body(stream: &mut TcpStream, request: &str, initial: &[u8]) -> io::Result<Vec<u8>> {
    let content_length = parse_content_length(request).unwrap_or(0);
    let header_len = find_header_end(initial).unwrap_or(initial.len());
    let already = if header_len <= initial.len() {
        &initial[header_len..]
    } else {
        &[][..]
    };

    let mut body = Vec::with_capacity(content_length.max(already.len()));
    body.extend_from_slice(already);

    if content_length > body.len() {
        let mut remaining = vec![0_u8; content_length - body.len()];
        stream.read_exact(&mut remaining)?;
        body.extend_from_slice(&remaining);
    }

    Ok(body)
}

fn parse_content_length(request: &str) -> Option<usize> {
    for line in request.lines() {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                return value.trim().parse::<usize>().ok();
            }
        }
    }
    None
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|chunk| chunk == b"\r\n\r\n")
        .map(|idx| idx + 4)
}

fn json_error(status: u16, reason: &'static str, message: String) -> JsonResponse {
    JsonResponse {
        status,
        reason,
        body: json!({ "error": message }),
    }
}

fn write_json_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    body: Value,
) -> io::Result<()> {
    let bytes = serde_json::to_vec(&body)
        .unwrap_or_else(|_| b"{\"error\":\"serialization failed\"}".to_vec());
    write_response(
        stream,
        status,
        reason,
        "application/json; charset=utf-8",
        &bytes,
    )
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    let headers = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

fn walk_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    walk_files_inner(dir, &mut files)?;
    Ok(files)
}

fn walk_files_inner(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_files_inner(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn to_url_path(path: &Path) -> String {
    let mut text = String::new();
    for component in path.components() {
        if !text.is_empty() {
            text.push('/');
        }
        text.push_str(&component.as_os_str().to_string_lossy());
    }
    text.replace('\\', "/")
}

fn to_lower_url_path(path: &Path) -> String {
    to_url_path(path).to_ascii_lowercase()
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn render_markdown_page(source_path: &Path, markdown: &str) -> io::Result<String> {
    let title = page_title(source_path, markdown);
    let body = render_markdown(markdown);

    Ok(format!(
        "<!doctype html>\
<html lang=\"en\">\
<head>\
<meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>{title}</title>\
<link rel=\"stylesheet\" href=\"/style.css\">\
</head>\
<body>\
<div class=\"site-shell\">\
<header class=\"site-header\">\
<a class=\"brand\" href=\"/\">neon_blue</a>\
<nav>\
<a href=\"/\">Home</a>\
<a href=\"/docs/\">Docs</a>\
</nav>\
</header>\
<main class=\"content markdown\">{body}</main>\
</div>\
</body>\
</html>"
    ))
}

fn page_title(path: &Path, markdown: &str) -> String {
    if let Some(title) = markdown.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("# ").map(|text| text.trim().to_string())
    }) {
        return title;
    }

    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace('_', " "))
        .unwrap_or_else(|| "Docs".to_string())
}

fn render_markdown(markdown: &str) -> String {
    let mut output = String::new();
    let mut lines = markdown.lines().peekable();
    let mut in_list = false;
    let mut in_code = false;
    let mut paragraph = Vec::new();

    while let Some(line) = lines.next() {
        let trimmed = line.trim_end();
        let blank = trimmed.trim().is_empty();

        if in_code {
            if trimmed.trim_start().starts_with("```") {
                output.push_str("</code></pre>");
                in_code = false;
            } else {
                output.push_str(&escape_html(trimmed));
                output.push('\n');
            }
            continue;
        }

        if trimmed.trim_start().starts_with("```") {
            flush_paragraph(&mut output, &mut paragraph);
            if in_list {
                output.push_str("</ul>");
                in_list = false;
            }
            output.push_str("<pre><code>");
            in_code = true;
            continue;
        }

        if blank {
            flush_paragraph(&mut output, &mut paragraph);
            if in_list {
                output.push_str("</ul>");
                in_list = false;
            }
            continue;
        }

        if let Some((level, heading)) = parse_heading(trimmed) {
            flush_paragraph(&mut output, &mut paragraph);
            if in_list {
                output.push_str("</ul>");
                in_list = false;
            }
            output.push_str(&format!("<h{level}>{}</h{level}>", render_inline(heading)));
            continue;
        }

        if let Some(item) = parse_list_item(trimmed) {
            flush_paragraph(&mut output, &mut paragraph);
            if !in_list {
                output.push_str("<ul>");
                in_list = true;
            }
            output.push_str("<li>");
            output.push_str(&render_inline(item));
            output.push_str("</li>");
            continue;
        }

        paragraph.push(trimmed.trim().to_string());
        if lines
            .peek()
            .map(|next| next.trim().is_empty())
            .unwrap_or(true)
        {
            flush_paragraph(&mut output, &mut paragraph);
            if in_list {
                output.push_str("</ul>");
                in_list = false;
            }
        }
    }

    flush_paragraph(&mut output, &mut paragraph);
    if in_list {
        output.push_str("</ul>");
    }
    if in_code {
        output.push_str("</code></pre>");
    }

    output
}

fn flush_paragraph(output: &mut String, paragraph: &mut Vec<String>) {
    if paragraph.is_empty() {
        return;
    }

    output.push_str("<p>");
    output.push_str(&render_inline(&paragraph.join(" ")));
    output.push_str("</p>");
    paragraph.clear();
}

fn parse_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }

    let rest = trimmed[level..].trim_start();
    if rest.is_empty() {
        return None;
    }

    Some((level, rest))
}

fn parse_list_item(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return Some(rest.trim());
    }

    let mut digits = 0;
    for ch in trimmed.chars() {
        if ch.is_ascii_digit() {
            digits += 1;
        } else {
            break;
        }
    }

    if digits == 0 {
        return None;
    }

    let remainder = &trimmed[digits..];
    remainder.strip_prefix(". ").map(str::trim)
}

fn render_inline(text: &str) -> String {
    let mut output = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut idx = 0;

    while idx < chars.len() {
        match chars[idx] {
            '`' => {
                if let Some(end) = find_char(&chars, idx + 1, '`') {
                    let content: String = chars[idx + 1..end].iter().collect();
                    output.push_str("<code>");
                    output.push_str(&escape_html(&content));
                    output.push_str("</code>");
                    idx = end + 1;
                } else {
                    output.push('`');
                    idx += 1;
                }
            }
            '[' => {
                if let Some(close_bracket) = find_char(&chars, idx + 1, ']') {
                    if close_bracket + 1 < chars.len() && chars[close_bracket + 1] == '(' {
                        if let Some(close_paren) = find_char(&chars, close_bracket + 2, ')') {
                            let label: String = chars[idx + 1..close_bracket].iter().collect();
                            let url: String =
                                chars[close_bracket + 2..close_paren].iter().collect();
                            let url = normalize_docs_link_url(&url);
                            output.push_str("<a href=\"");
                            output.push_str(&escape_html(&url));
                            output.push_str("\">");
                            output.push_str(&escape_html(&label));
                            output.push_str("</a>");
                            idx = close_paren + 1;
                            continue;
                        }
                    }
                }
                output.push('[');
                idx += 1;
            }
            '&' | '<' | '>' | '"' => {
                output.push_str(&escape_html_char(chars[idx]));
                idx += 1;
            }
            ch => {
                output.push(ch);
                idx += 1;
            }
        }
    }

    output
}

fn find_char(chars: &[char], start: usize, needle: char) -> Option<usize> {
    chars[start..]
        .iter()
        .position(|ch| *ch == needle)
        .map(|offset| start + offset)
}

fn escape_html(text: &str) -> String {
    let mut output = String::new();
    for ch in text.chars() {
        output.push_str(&escape_html_char(ch));
    }
    output
}

fn escape_html_char(ch: char) -> String {
    match ch {
        '&' => "&amp;".to_string(),
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '"' => "&quot;".to_string(),
        '\'' => "&#39;".to_string(),
        _ => ch.to_string(),
    }
}

fn normalize_docs_link_url(url: &str) -> String {
    if url.starts_with('#') || url.contains("://") || url.starts_with("mailto:") {
        return url.to_string();
    }

    url.to_ascii_lowercase()
}
