use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::Local;
use serde::{Deserialize, Serialize};
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};

const CACHE_VERSION: u32 = 1;
const CACHE_SAVE_DEBOUNCE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedApp {
    pub identity: String,
    pub exe_name: String,
    pub app_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedAppEntry {
    pub exe_name: String,
    pub app_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_version: Option<String>,
    pub source: String,
    pub saved_on: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppNameCacheFile {
    pub version: u32,
    pub apps: HashMap<String, CachedAppEntry>,
}

impl Default for AppNameCacheFile {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            apps: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct VersionMetadata {
    product_name: Option<String>,
    company_name: Option<String>,
    product_version: Option<String>,
}

#[derive(Debug, Clone)]
struct AppxManifestMetadata {
    display_name: Option<String>,
    publisher_display_name: Option<String>,
    identity_name: Option<String>,
}

pub struct AppNameResolver {
    cache_path: PathBuf,
    user_exe_names: HashMap<String, String>,
    cache: AppNameCacheFile,
    dirty: bool,
    dirty_since: Option<Instant>,
}

impl AppNameResolver {
    pub fn new(
        cache_path: PathBuf,
        user_exe_names: HashMap<String, String>,
    ) -> Result<Self, String> {
        let cache = load_cache_file(&cache_path)?;

        Ok(Self {
            cache_path,
            user_exe_names: normalize_exe_name_map(user_exe_names),
            cache,
            dirty: false,
            dirty_since: None,
        })
    }

    pub fn resolve_app_name(&mut self, exe_path: &str) -> ResolvedApp {
        let identity = normalize_exe_path(exe_path);
        let exe_name = exe_name_from_path(&identity).unwrap_or_else(|| identity.clone());

        // User-authored exe labels should always win, even if we previously cached
        // metadata or fallback results for a specific path.
        if let Some(app_name) = lookup_exe_label(&self.user_exe_names, &exe_name) {
            return ResolvedApp {
                identity,
                exe_name,
                app_name,
            };
        }

        if let Some(entry) = self.cache.apps.get(&identity) {
            return ResolvedApp {
                identity,
                exe_name: entry.exe_name.clone(),
                app_name: entry.app_name.clone(),
            };
        }

        if let Some(metadata) = read_version_metadata(exe_path) {
            if let Some(product_name) = metadata
                .product_name
                .as_deref()
                .filter(|value| is_valid_product_name(value, &exe_name))
            {
                let app_name = product_name.trim().to_string();
                self.cache_resolved_name(
                    &identity,
                    &exe_name,
                    &app_name,
                    metadata.company_name,
                    metadata.product_version,
                    "version_info",
                );

                return ResolvedApp {
                    identity,
                    exe_name,
                    app_name,
                };
            }
        }

        if let Some(metadata) = read_appx_manifest_metadata(exe_path) {
            if let Some(display_name) = metadata
                .display_name
                .as_deref()
                .filter(|value| is_valid_appx_display_name(value, &exe_name))
            {
                let app_name = display_name.trim().to_string();
                self.cache_resolved_name(
                    &identity,
                    &exe_name,
                    &app_name,
                    metadata.publisher_display_name,
                    metadata.identity_name,
                    "appx_manifest",
                );

                return ResolvedApp {
                    identity,
                    exe_name,
                    app_name,
                };
            }
        }

        let fallback = prettify_exe_name(&exe_name);
        self.cache_resolved_name(&identity, &exe_name, &fallback, None, None, "fallback");

        ResolvedApp {
            identity,
            exe_name,
            app_name: fallback,
        }
    }

    pub fn set_user_exe_names(&mut self, exe_names: HashMap<String, String>) {
        self.user_exe_names = normalize_exe_name_map(exe_names);
    }

    pub fn flush_if_due(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }

        let due = self
            .dirty_since
            .map(|timestamp| timestamp.elapsed() >= CACHE_SAVE_DEBOUNCE)
            .unwrap_or(false);

        if due {
            self.save_if_dirty()?;
        }

        Ok(())
    }

    pub fn save_if_dirty(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }

        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }

        self.cache.version = CACHE_VERSION;
        let raw = serde_json::to_string_pretty(&self.cache).map_err(|err| err.to_string())?;
        std::fs::write(&self.cache_path, raw).map_err(|err| err.to_string())?;
        self.dirty = false;
        self.dirty_since = None;
        Ok(())
    }

    fn cache_resolved_name(
        &mut self,
        identity: &str,
        exe_name: &str,
        app_name: &str,
        company_name: Option<String>,
        product_version: Option<String>,
        source: &str,
    ) {
        let entry = CachedAppEntry {
            exe_name: exe_name.to_string(),
            app_name: app_name.to_string(),
            company_name,
            product_version,
            source: source.to_string(),
            saved_on: Local::now().format("%Y-%m-%d").to_string(),
        };

        let needs_update = self.cache.apps.get(identity) != Some(&entry);
        if needs_update {
            self.cache.apps.insert(identity.to_string(), entry);
            self.dirty = true;
            self.dirty_since = Some(Instant::now());
        }
    }
}

pub fn default_cache_path(base_dir: impl AsRef<Path>) -> PathBuf {
    base_dir.as_ref().join("app_name_cache.json")
}

pub fn normalize_exe_path(value: &str) -> String {
    value.trim().replace('/', "\\").to_lowercase()
}

pub fn normalize_exe_name_map(map: HashMap<String, String>) -> HashMap<String, String> {
    map.into_iter()
        .filter_map(|(key, value)| {
            let normalized_key = key.trim().to_lowercase();
            let normalized_value = value.trim().to_string();

            if normalized_key.is_empty() || normalized_value.is_empty() {
                None
            } else {
                Some((normalized_key, normalized_value))
            }
        })
        .collect()
}

pub fn prettify_exe_name(exe_name: &str) -> String {
    match exe_name.trim().to_lowercase().as_str() {
        "unknown" => "Unknown".into(),
        "idle" => "Idle".into(),
        _ => exe_name
            .trim_end_matches(".exe")
            .split(['-', '_', '.', ' '])
            .filter(|part| !part.is_empty())
            .map(capitalize)
            .collect::<Vec<_>>()
            .join(" "),
    }
}

pub fn looks_like_exe_path(value: &str) -> bool {
    let normalized = normalize_exe_path(value);
    normalized.contains('\\') && normalized.ends_with(".exe")
}

fn load_cache_file(path: &Path) -> Result<AppNameCacheFile, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    if !path.exists() {
        return Ok(AppNameCacheFile::default());
    }

    let raw = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    if raw.trim().is_empty() {
        return Ok(AppNameCacheFile::default());
    }

    let mut cache = serde_json::from_str::<AppNameCacheFile>(&raw).map_err(|err| err.to_string())?;
    if cache.version != CACHE_VERSION {
        cache.version = CACHE_VERSION;
    }

    cache.apps = cache
        .apps
        .into_iter()
        .filter_map(|(path, entry)| {
            let identity = normalize_exe_path(&path);
            let exe_name = entry.exe_name.trim().to_lowercase();
            let app_name = entry.app_name.trim().to_string();
            let source = entry.source.trim().to_string();

            if identity.is_empty() || exe_name.is_empty() || app_name.is_empty() || source.is_empty() {
                None
            } else {
                Some((
                    identity,
                    CachedAppEntry {
                        exe_name,
                        app_name,
                        company_name: entry.company_name.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()),
                        product_version: entry.product_version.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()),
                        source,
                        saved_on: if entry.saved_on.trim().is_empty() {
                            Local::now().format("%Y-%m-%d").to_string()
                        } else {
                            entry.saved_on
                        },
                    },
                ))
            }
        })
        .collect();

    Ok(cache)
}

fn lookup_exe_label(labels: &HashMap<String, String>, exe_name: &str) -> Option<String> {
    let target = exe_name.trim().to_lowercase();
    let target_no_ext = target.trim_end_matches(".exe");

    if let Some(label) = labels.get(&target) {
        return Some(label.clone());
    }

    for (key, value) in labels {
        let key_no_ext = key.trim_end_matches(".exe");
        if key == &target || key_no_ext == target_no_ext || key == target_no_ext || key_no_ext == target {
            return Some(value.clone());
        }
    }

    None
}

fn exe_name_from_path(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_lowercase())
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn is_valid_product_name(value: &str, exe_name: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    let normalized = trimmed.to_lowercase();
    let exe_name_no_ext = exe_name.trim_end_matches(".exe").to_lowercase();
    let invalid_values = ["application", "electron", "updater", "setup", "launcher"];

    if invalid_values.contains(&normalized.as_str()) {
        return false;
    }

    if normalized == exe_name.to_lowercase() || normalized == exe_name_no_ext {
        return false;
    }

    if trimmed.contains('\\') || trimmed.contains('/') || normalized.ends_with(".exe") {
        return false;
    }

    trimmed.chars().any(|char| char.is_alphanumeric())
}

fn is_valid_appx_display_name(value: &str, exe_name: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    let normalized = trimmed.to_lowercase();
    if normalized.starts_with("ms-resource:") {
        return false;
    }

    let exe_name_no_ext = exe_name.trim_end_matches(".exe").to_lowercase();
    let invalid_values = ["application", "electron", "updater", "setup", "launcher"];

    if invalid_values.contains(&normalized.as_str()) {
        return false;
    }

    if trimmed.contains('\\') || trimmed.contains('/') || normalized.ends_with(".exe") {
        return false;
    }

    // Unlike PE ProductName, an Appx DisplayName can legitimately differ only by
    // capitalization and branding punctuation from the executable stem.
    if normalized == exe_name_no_ext {
        return trimmed != exe_name.trim_end_matches(".exe");
    }

    trimmed.chars().any(|char| char.is_alphanumeric())
}

fn read_version_metadata(path: &str) -> Option<VersionMetadata> {
    let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut handle = 0u32;
    let info_size = unsafe { GetFileVersionInfoSizeW(PCWSTR(wide_path.as_ptr()), Some(&mut handle)) };
    if info_size == 0 {
        return None;
    }

    let mut buffer = vec![0u8; info_size as usize];
    let ok = unsafe {
        GetFileVersionInfoW(
            PCWSTR(wide_path.as_ptr()),
            Some(0),
            info_size,
            buffer.as_mut_ptr() as *mut c_void,
        )
    };
    if ok.is_err() {
        return None;
    }

    let language_pairs = version_translations(&buffer);
    let mut product_name = None;
    let mut company_name = None;
    let mut product_version = None;

    for (language, code_page) in language_pairs {
        if product_name.is_none() {
            product_name = query_version_string(&buffer, language, code_page, "ProductName");
        }
        if company_name.is_none() {
            company_name = query_version_string(&buffer, language, code_page, "CompanyName");
        }
        if product_version.is_none() {
            product_version = query_version_string(&buffer, language, code_page, "ProductVersion");
        }
    }

    if product_name.is_none() {
        product_name = query_version_string(&buffer, 0x0409, 0x04B0, "ProductName");
    }
    if company_name.is_none() {
        company_name = query_version_string(&buffer, 0x0409, 0x04B0, "CompanyName");
    }
    if product_version.is_none() {
        product_version = query_version_string(&buffer, 0x0409, 0x04B0, "ProductVersion");
    }

    if product_name.is_none() && company_name.is_none() && product_version.is_none() {
        return None;
    }

    Some(VersionMetadata {
        product_name,
        company_name,
        product_version,
    })
}

fn read_appx_manifest_metadata(exe_path: &str) -> Option<AppxManifestMetadata> {
    let manifest_path = find_appx_manifest_path(exe_path)?;
    let raw = std::fs::read_to_string(manifest_path).ok()?;

    Some(AppxManifestMetadata {
        display_name: extract_xml_tag_value(&raw, "DisplayName"),
        publisher_display_name: extract_xml_tag_value(&raw, "PublisherDisplayName"),
        identity_name: extract_identity_name(&raw),
    })
}

fn find_appx_manifest_path(exe_path: &str) -> Option<PathBuf> {
    let exe_path = Path::new(exe_path);
    let mut current = exe_path.parent()?;

    // We walk upward so Store/MSIX layouts under WindowsApps and unpacked Appx installs
    // both get a chance to resolve from AppxManifest.xml without changing the main pipeline.
    loop {
        let manifest_path = current.join("AppxManifest.xml");
        if manifest_path.is_file() {
            return Some(manifest_path);
        }

        current = current.parent()?;
    }
}

fn extract_xml_tag_value(xml: &str, tag_name: &str) -> Option<String> {
    let start_tag = format!("<{tag_name}>");
    let end_tag = format!("</{tag_name}>");
    let start = xml.find(&start_tag)? + start_tag.len();
    let rest = &xml[start..];
    let end = rest.find(&end_tag)?;
    let value = rest[..end].trim();

    if value.is_empty() {
        None
    } else {
        Some(xml_unescape(value))
    }
}

fn extract_identity_name(xml: &str) -> Option<String> {
    let identity_start = xml.find("<Identity")?;
    let rest = &xml[identity_start..];
    let close = rest.find('>')?;
    extract_xml_attribute_value(&rest[..close], "Name")
}

fn extract_xml_attribute_value(element: &str, attribute_name: &str) -> Option<String> {
    let marker = format!("{attribute_name}=\"");
    let start = element.find(&marker)? + marker.len();
    let rest = &element[start..];
    let end = rest.find('"')?;
    let value = rest[..end].trim();

    if value.is_empty() {
        None
    } else {
        Some(xml_unescape(value))
    }
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn version_translations(buffer: &[u8]) -> Vec<(u16, u16)> {
    let query: Vec<u16> = "\\VarFileInfo\\Translation\0".encode_utf16().collect();
    let mut value = std::ptr::null_mut::<c_void>();
    let mut len = 0u32;

    let ok = unsafe {
        VerQueryValueW(
            buffer.as_ptr() as *const c_void,
            PCWSTR(query.as_ptr()),
            &mut value,
            &mut len,
        )
    };

    if !ok.as_bool() || value.is_null() || len < 4 {
        return vec![(0x0409, 0x04B0)];
    }

    let bytes = unsafe { std::slice::from_raw_parts(value as *const u8, len as usize) };
    let mut translations = Vec::new();
    for chunk in bytes.chunks_exact(4) {
        let language = u16::from_le_bytes([chunk[0], chunk[1]]);
        let code_page = u16::from_le_bytes([chunk[2], chunk[3]]);
        translations.push((language, code_page));
    }

    if translations.is_empty() {
        translations.push((0x0409, 0x04B0));
    }

    translations
}

fn query_version_string(
    buffer: &[u8],
    language: u16,
    code_page: u16,
    key: &str,
) -> Option<String> {
    let query = format!("\\StringFileInfo\\{language:04x}{code_page:04x}\\{key}\0");
    let wide_query: Vec<u16> = query.encode_utf16().collect();
    let mut value = std::ptr::null_mut::<u16>();
    let mut len = 0u32;

    let ok = unsafe {
        VerQueryValueW(
            buffer.as_ptr() as *const c_void,
            PCWSTR(wide_query.as_ptr()),
            &mut value as *mut _ as *mut *mut c_void,
            &mut len,
        )
    };

    if !ok.as_bool() || value.is_null() || len == 0 {
        return None;
    }

    let slice = unsafe { std::slice::from_raw_parts(value, len as usize) };
    let string = String::from_utf16_lossy(slice)
        .trim_end_matches('\0')
        .trim()
        .to_string();

    if string.is_empty() {
        None
    } else {
        Some(string)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        extract_identity_name, extract_xml_tag_value, is_valid_appx_display_name,
        is_valid_product_name, normalize_exe_path, prettify_exe_name, AppNameCacheFile,
        AppNameResolver, CachedAppEntry,
    };
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn normalizes_paths_to_lowercase_backslashes() {
        assert_eq!(
            normalize_exe_path("C:/Program Files/Google/Chrome/Application/CHROME.EXE"),
            "c:\\program files\\google\\chrome\\application\\chrome.exe"
        );
    }

    #[test]
    fn preserves_existing_prettify_behavior() {
        assert_eq!(prettify_exe_name("code.exe"), "Code");
        assert_eq!(prettify_exe_name("minecraft-launcher.exe"), "Minecraft Launcher");
        assert_eq!(prettify_exe_name("obs64.exe"), "Obs64");
    }

    #[test]
    fn rejects_generic_product_names() {
        assert!(!is_valid_product_name("Electron", "discord.exe"));
        assert!(!is_valid_product_name("launcher", "minecraft-launcher.exe"));
        assert!(is_valid_product_name("Google Chrome", "chrome.exe"));
    }

    #[test]
    fn rejects_unresolved_ms_resource_display_names() {
        assert!(!is_valid_appx_display_name("ms-resource:AppName", "chatgpt.exe"));
        assert!(is_valid_appx_display_name("TradingView", "tradingview.exe"));
    }

    #[test]
    fn extracts_manifest_values() {
        let manifest = r#"
        <Package>
            <Identity Name="AnthropicClaude" Publisher="CN=Anthropic" />
            <Properties>
                <DisplayName>Claude</DisplayName>
                <PublisherDisplayName>Anthropic</PublisherDisplayName>
            </Properties>
        </Package>
        "#;

        assert_eq!(extract_xml_tag_value(manifest, "DisplayName").as_deref(), Some("Claude"));
        assert_eq!(
            extract_xml_tag_value(manifest, "PublisherDisplayName").as_deref(),
            Some("Anthropic")
        );
        assert_eq!(extract_identity_name(manifest).as_deref(), Some("AnthropicClaude"));
    }

    #[test]
    fn user_exe_labels_override_cached_names() {
        let mut resolver = AppNameResolver {
            cache_path: PathBuf::from("app_name_cache.json"),
            user_exe_names: HashMap::from([("chatgpt.exe".to_string(), "ChatGPT".to_string())]),
            cache: AppNameCacheFile {
                version: 1,
                apps: HashMap::from([(
                    "c:\\program files\\windowsapps\\chatgpt\\chatgpt.exe".to_string(),
                    CachedAppEntry {
                        exe_name: "chatgpt.exe".to_string(),
                        app_name: "Chatgpt".to_string(),
                        company_name: None,
                        product_version: None,
                        source: "fallback".to_string(),
                        saved_on: "2026-05-18".to_string(),
                    },
                )]),
            },
            dirty: false,
            dirty_since: None,
        };

        let resolved = resolver.resolve_app_name("C:/Program Files/WindowsApps/ChatGPT/ChatGPT.exe");
        assert_eq!(resolved.app_name, "ChatGPT");
    }
}
