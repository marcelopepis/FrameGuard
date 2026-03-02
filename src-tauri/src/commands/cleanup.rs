//! Módulo de limpeza de sistema — escaneamento e remoção de arquivos temporários,
//! caches de GPU/browsers/apps e itens avançados como Windows.old e WinSxS.
//!
//! Dois comandos Tauri:
//!   - `scan_cleanup`: escaneia o sistema e retorna categorias com tamanhos reais
//!   - `execute_cleanup`: remove os itens selecionados com progresso em tempo real

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tauri::Emitter;

use crate::commands::health_check::LockingProcessInfo;
use crate::commands::system_info;
use crate::utils::command_runner::run_powershell;
use crate::utils::file_locks;

// ─── Tipos públicos ──────────────────────────────────────────────────────────

/// Nível de risco da categoria de limpeza.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CleanupRisk {
    /// Seguro — sempre regenerado pelo sistema
    Safe,
    /// Atenção — pode causar inconveniência menor
    Moderate,
    /// Cuidado — pode afetar software instalado
    Caution,
}

/// Um item individual escaneável dentro de uma categoria.
#[derive(Debug, Clone, Serialize)]
pub struct CleanupItem {
    pub id: String,
    pub name: String,
    pub path_display: String,
    pub size_bytes: u64,
    pub file_count: u32,
    /// Se este item é marcado por padrão na UI
    pub default_selected: bool,
}

/// Grupo de itens de limpeza relacionados.
#[derive(Debug, Clone, Serialize)]
pub struct CleanupCategory {
    pub id: String,
    pub name: String,
    pub description: String,
    pub risk: CleanupRisk,
    pub default_selected: bool,
    pub items: Vec<CleanupItem>,
    pub total_size_bytes: u64,
    pub total_file_count: u32,
}

/// Resultado completo do scan.
#[derive(Debug, Clone, Serialize)]
pub struct CleanupScanResult {
    pub categories: Vec<CleanupCategory>,
    pub total_size_bytes: u64,
    pub total_file_count: u32,
    pub scan_duration_seconds: u64,
}

/// Evento de progresso emitido durante a limpeza.
#[derive(Debug, Clone, Serialize)]
pub struct CleanupProgressEvent {
    pub current_category: String,
    pub current_item: String,
    pub progress_percent: f64,
    pub freed_bytes_so_far: u64,
    pub message: String,
}

/// Resultado individual por item limpo.
#[derive(Debug, Clone, Serialize)]
pub struct CleanupItemResult {
    pub id: String,
    pub name: String,
    pub freed_bytes: u64,
    pub files_removed: u32,
    pub files_skipped: u32,
    pub errors: Vec<String>,
}

/// Resultado final da operação de limpeza.
#[derive(Debug, Clone, Serialize)]
pub struct CleanupResult {
    pub total_freed_bytes: u64,
    pub total_files_removed: u32,
    pub total_files_skipped: u32,
    pub duration_seconds: u64,
    pub item_results: Vec<CleanupItemResult>,
    pub locking_processes: Vec<LockingProcessInfo>,
}

// ─── Estruturas internas ─────────────────────────────────────────────────────

/// Resultado da remoção de conteúdo de um diretório.
struct DeleteResult {
    errors: Vec<String>,
    locked_paths: Vec<String>,
    files_removed: u32,
    files_skipped: u32,
}

// ─── Helpers de filesystem ───────────────────────────────────────────────────

/// Calcula recursivamente size + file_count de um caminho.
fn scan_dir_stats(path: &Path) -> (u64, u32) {
    if !path.exists() {
        return (0, 0);
    }
    if path.is_file() {
        return (std::fs::metadata(path).map(|m| m.len()).unwrap_or(0), 1);
    }
    let mut size: u64 = 0;
    let mut count: u32 = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            let (s, c) = scan_dir_stats(&p);
            size += s;
            count += c;
        }
    }
    (size, count)
}

/// Escaneia múltiplos caminhos e retorna totais combinados.
fn scan_paths_stats(paths: &[PathBuf]) -> (u64, u32) {
    let mut total_size: u64 = 0;
    let mut total_count: u32 = 0;
    for p in paths {
        let (s, c) = scan_dir_stats(p);
        total_size += s;
        total_count += c;
    }
    (total_size, total_count)
}

/// Remove conteúdo de um diretório recursivamente, pulando arquivos travados.
fn delete_dir_contents(path: &Path, freed: &mut u64) -> DeleteResult {
    let mut errors: Vec<String> = Vec::new();
    let mut locked_paths: Vec<String> = Vec::new();
    let mut files_removed: u32 = 0;
    let mut files_skipped: u32 = 0;

    if !path.exists() {
        return DeleteResult { errors, locked_paths, files_removed, files_skipped };
    }

    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => {
            errors.push(format!("Erro ao abrir {}: {}", path.display(), e));
            return DeleteResult { errors, locked_paths, files_removed, files_skipped };
        }
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();
        let size = scan_dir_stats(&entry_path).0;

        let result = if entry_path.is_dir() {
            std::fs::remove_dir_all(&entry_path)
        } else {
            std::fs::remove_file(&entry_path)
        };

        match result {
            Ok(()) => {
                *freed += size;
                files_removed += 1;
            }
            Err(e) => {
                files_skipped += 1;
                let raw_err = e.raw_os_error();
                let is_locked = raw_err == Some(32) || raw_err == Some(5);

                if is_locked && entry_path.is_file() {
                    let full_path = entry_path.to_string_lossy().to_string();
                    let procs = file_locks::get_locking_processes(&full_path);
                    if !procs.is_empty() {
                        locked_paths.push(full_path);
                    } else {
                        locked_paths.push(entry_path.to_string_lossy().to_string());
                    }
                } else if is_locked {
                    locked_paths.push(entry_path.to_string_lossy().to_string());
                }

                errors.push(format!(
                    "{}: {}",
                    entry_path.file_name().unwrap_or_default().to_string_lossy(),
                    e,
                ));
            }
        }
    }

    DeleteResult { errors, locked_paths, files_removed, files_skipped }
}

/// Deleta conteúdo de múltiplos paths, acumulando resultados.
fn delete_paths_contents(paths: &[PathBuf], freed: &mut u64) -> DeleteResult {
    let mut all_errors = Vec::new();
    let mut all_locked = Vec::new();
    let mut total_removed: u32 = 0;
    let mut total_skipped: u32 = 0;

    for p in paths {
        let r = delete_dir_contents(p, freed);
        all_errors.extend(r.errors);
        all_locked.extend(r.locked_paths);
        total_removed += r.files_removed;
        total_skipped += r.files_skipped;
    }

    DeleteResult {
        errors: all_errors,
        locked_paths: all_locked,
        files_removed: total_removed,
        files_skipped: total_skipped,
    }
}

/// Agrupa processos que travam arquivos por PID.
fn aggregate_locking_processes(locked_paths: &[String]) -> Vec<LockingProcessInfo> {
    use std::collections::HashMap;

    let mut proc_map: HashMap<u32, (String, usize)> = HashMap::new();

    for path in locked_paths {
        let procs = file_locks::get_locking_processes(path);
        for p in procs {
            let entry = proc_map.entry(p.pid).or_insert_with(|| (p.name.clone(), 0));
            entry.1 += 1;
        }
    }

    let mut result: Vec<LockingProcessInfo> = proc_map
        .into_iter()
        .map(|(pid, (name, count))| LockingProcessInfo {
            pid,
            name,
            file_count: count,
        })
        .collect();

    result.sort_by(|a, b| b.file_count.cmp(&a.file_count));
    result
}

// ─── Helpers de path ─────────────────────────────────────────────────────────

fn env_path(var: &str) -> PathBuf {
    PathBuf::from(std::env::var(var).unwrap_or_default())
}

fn local_app_data() -> PathBuf {
    env_path("LOCALAPPDATA")
}

fn app_data() -> PathBuf {
    env_path("APPDATA")
}

fn temp_dir() -> PathBuf {
    env_path("TEMP")
}

// ─── Scan por categoria ──────────────────────────────────────────────────────

fn scan_sistema_windows() -> CleanupCategory {
    let local = local_app_data();

    let mut items = Vec::new();

    // %TEMP%
    let temp = temp_dir();
    let (s, c) = scan_dir_stats(&temp);
    if s > 0 {
        items.push(CleanupItem {
            id: "temp_user".into(), name: "Temp do Usuário".into(),
            path_display: temp.to_string_lossy().into(), size_bytes: s, file_count: c,
            default_selected: true,
        });
    }

    // Windows\Temp
    let win_temp = PathBuf::from(r"C:\Windows\Temp");
    let (s, c) = scan_dir_stats(&win_temp);
    if s > 0 {
        items.push(CleanupItem {
            id: "temp_windows".into(), name: "Windows Temp".into(),
            path_display: r"C:\Windows\Temp".into(), size_bytes: s, file_count: c,
            default_selected: true,
        });
    }

    // Windows Error Reports
    let wer_paths = vec![
        local.join(r"Microsoft\Windows\WER\ReportArchive"),
        local.join(r"Microsoft\Windows\WER\ReportQueue"),
        PathBuf::from(r"C:\ProgramData\Microsoft\Windows\WER"),
    ];
    let (s, c) = scan_paths_stats(&wer_paths);
    if s > 0 {
        items.push(CleanupItem {
            id: "wer_reports".into(), name: "Windows Error Reports".into(),
            path_display: "WER ReportArchive + ReportQueue".into(), size_bytes: s, file_count: c,
            default_selected: true,
        });
    }

    // Windows Update Cache
    let wu_path = PathBuf::from(r"C:\Windows\SoftwareDistribution\Download");
    let (s, c) = scan_dir_stats(&wu_path);
    if s > 0 {
        items.push(CleanupItem {
            id: "wu_cache".into(), name: "Windows Update Cache".into(),
            path_display: r"SoftwareDistribution\Download".into(), size_bytes: s, file_count: c,
            default_selected: true,
        });
    }

    // Delivery Optimization Cache
    let do_path = PathBuf::from(
        r"C:\Windows\ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache"
    );
    let (s, c) = scan_dir_stats(&do_path);
    if s > 0 {
        items.push(CleanupItem {
            id: "delivery_optim".into(), name: "Delivery Optimization Cache".into(),
            path_display: "DeliveryOptimization Cache".into(), size_bytes: s, file_count: c,
            default_selected: true,
        });
    }

    // Thumbnail Cache
    let explorer_dir = local.join(r"Microsoft\Windows\Explorer");
    let (thumb_size, thumb_count) = scan_thumbcache(&explorer_dir);
    if thumb_size > 0 {
        items.push(CleanupItem {
            id: "thumbcache".into(), name: "Thumbnail Cache".into(),
            path_display: "thumbcache_*.db + iconcache_*.db".into(),
            size_bytes: thumb_size, file_count: thumb_count,
            default_selected: true,
        });
    }

    // Memory Dumps
    let dump_paths = vec![
        PathBuf::from(r"C:\Windows\MEMORY.DMP"),
        PathBuf::from(r"C:\Windows\Minidump"),
    ];
    let (s, c) = scan_paths_stats(&dump_paths);
    if s > 0 {
        items.push(CleanupItem {
            id: "memory_dumps".into(), name: "Memory Dumps".into(),
            path_display: "MEMORY.DMP + Minidump".into(), size_bytes: s, file_count: c,
            default_selected: true,
        });
    }

    // CBS/DISM/Upgrade Logs
    let log_paths = vec![
        PathBuf::from(r"C:\Windows\Logs\CBS"),
        PathBuf::from(r"C:\Windows\Logs\DISM"),
        PathBuf::from(r"C:\Windows\Logs\WindowsUpdate"),
    ];
    let (s, c) = scan_paths_stats(&log_paths);
    if s > 0 {
        items.push(CleanupItem {
            id: "cbs_logs".into(), name: "CBS/DISM/Upgrade Logs".into(),
            path_display: r"Windows\Logs\CBS + DISM + WindowsUpdate".into(),
            size_bytes: s, file_count: c,
            default_selected: true,
        });
    }

    // Recycle Bin
    let recycle_size = get_recycle_bin_size();
    if recycle_size > 0 {
        items.push(CleanupItem {
            id: "recycle_bin".into(), name: "Lixeira".into(),
            path_display: "Lixeira do Windows".into(),
            size_bytes: recycle_size, file_count: 0,
            default_selected: true,
        });
    }

    // Downloaded Program Files
    let dpf = PathBuf::from(r"C:\Windows\Downloaded Program Files");
    let (s, c) = scan_dir_stats(&dpf);
    if s > 0 {
        items.push(CleanupItem {
            id: "downloaded_programs".into(), name: "Downloaded Program Files".into(),
            path_display: r"C:\Windows\Downloaded Program Files".into(),
            size_bytes: s, file_count: c,
            default_selected: true,
        });
    }

    let total_size: u64 = items.iter().map(|i| i.size_bytes).sum();
    let total_count: u32 = items.iter().map(|i| i.file_count).sum();

    CleanupCategory {
        id: "sistema_windows".into(),
        name: "Sistema Windows".into(),
        description: "Arquivos temporários, logs e caches do sistema".into(),
        risk: CleanupRisk::Safe,
        default_selected: true,
        items,
        total_size_bytes: total_size,
        total_file_count: total_count,
    }
}

/// Escaneia thumbcache_*.db e iconcache_*.db no diretório Explorer.
fn scan_thumbcache(explorer_dir: &Path) -> (u64, u32) {
    let mut size: u64 = 0;
    let mut count: u32 = 0;
    if let Ok(entries) = std::fs::read_dir(explorer_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.starts_with("thumbcache_") && name.ends_with(".db")
                || name.starts_with("iconcache_") && name.ends_with(".db")
            {
                size += std::fs::metadata(entry.path()).map(|m| m.len()).unwrap_or(0);
                count += 1;
            }
        }
    }
    (size, count)
}

/// Obtém tamanho da Lixeira via PowerShell COM.
fn get_recycle_bin_size() -> u64 {
    let script = r#"
        try {
            $shell = New-Object -ComObject Shell.Application
            $bin = $shell.NameSpace(10)
            $total = 0
            foreach ($item in $bin.Items()) {
                $total += $item.Size
            }
            Write-Output $total
        } catch {
            Write-Output 0
        }
    "#;
    match run_powershell(script) {
        Ok(out) => out.stdout.trim().parse::<u64>().unwrap_or(0),
        Err(_) => 0,
    }
}

fn scan_gpu_shader_cache() -> CleanupCategory {
    let vendor = system_info::detect_gpu_vendor_sync();
    let local = local_app_data();
    let appdata = app_data();

    let mut items = Vec::new();

    // NVIDIA
    if vendor == "nvidia" || vendor == "unknown" {
        let nvidia_paths = vec![
            ("nvidia_dxcache", "NVIDIA DXCache", local.join(r"NVIDIA\DXCache")),
            ("nvidia_glcache", "NVIDIA GLCache", local.join(r"NVIDIA\GLCache")),
            ("nvidia_computecache", "NVIDIA ComputeCache", appdata.join(r"NVIDIA\ComputeCache")),
        ];
        for (id, name, path) in nvidia_paths {
            let (s, c) = scan_dir_stats(&path);
            if s > 0 {
                items.push(CleanupItem {
                    id: id.into(), name: name.into(),
                    path_display: path.to_string_lossy().into(),
                    size_bytes: s, file_count: c,
                    default_selected: true,
                });
            }
        }
    }

    // AMD
    if vendor == "amd" || vendor == "unknown" {
        let amd_paths = vec![
            ("amd_dxcache", "AMD DxCache", local.join(r"AMD\DxCache")),
            ("amd_dxccache", "AMD DxcCache", local.join(r"AMD\DxcCache")),
            ("amd_glcache", "AMD GLCache", local.join(r"AMD\GLCache")),
            ("amd_vkcache", "AMD VkCache", local.join(r"AMD\VkCache")),
        ];
        for (id, name, path) in amd_paths {
            let (s, c) = scan_dir_stats(&path);
            if s > 0 {
                items.push(CleanupItem {
                    id: id.into(), name: name.into(),
                    path_display: path.to_string_lossy().into(),
                    size_bytes: s, file_count: c,
                    default_selected: true,
                });
            }
        }
    }

    // Intel
    if vendor == "intel" || vendor == "unknown" {
        let path = local.join(r"Intel\ShaderCache");
        let (s, c) = scan_dir_stats(&path);
        if s > 0 {
            items.push(CleanupItem {
                id: "intel_shadercache".into(), name: "Intel ShaderCache".into(),
                path_display: path.to_string_lossy().into(),
                size_bytes: s, file_count: c,
                default_selected: true,
            });
        }
    }

    // DirectX D3DSCache (sempre)
    let d3ds = local.join("D3DSCache");
    let (s, c) = scan_dir_stats(&d3ds);
    if s > 0 {
        items.push(CleanupItem {
            id: "dx_d3dscache".into(), name: "DirectX D3DSCache".into(),
            path_display: d3ds.to_string_lossy().into(),
            size_bytes: s, file_count: c,
            default_selected: true,
        });
    }

    let total_size: u64 = items.iter().map(|i| i.size_bytes).sum();
    let total_count: u32 = items.iter().map(|i| i.file_count).sum();

    CleanupCategory {
        id: "gpu_shader_cache".into(),
        name: "GPU Shader Cache".into(),
        description: "Pode causar stutter temporário no primeiro launch de jogos".into(),
        risk: CleanupRisk::Moderate,
        default_selected: true,
        items,
        total_size_bytes: total_size,
        total_file_count: total_count,
    }
}

/// Escaneia caches de browsers Chromium (Chrome, Edge, Brave, Opera).
fn scan_chromium_browser(
    id_prefix: &str, name: &str, user_data_dir: &Path,
) -> Vec<CleanupItem> {
    let mut items = Vec::new();
    if !user_data_dir.exists() {
        return items;
    }

    // Encontra perfis: Default, Profile 1, Profile 2, etc.
    let profiles = find_chromium_profiles(user_data_dir);
    let mut total_size: u64 = 0;
    let mut total_count: u32 = 0;
    let mut cache_paths: Vec<PathBuf> = Vec::new();

    for profile_dir in &profiles {
        let cache_dirs = vec![
            profile_dir.join(r"Cache\Cache_Data"),
            profile_dir.join("Code Cache"),
            profile_dir.join("GPUCache"),
            profile_dir.join(r"Service Worker\CacheStorage"),
        ];
        for cd in cache_dirs {
            let (s, c) = scan_dir_stats(&cd);
            total_size += s;
            total_count += c;
            if s > 0 {
                cache_paths.push(cd);
            }
        }
    }

    if total_size > 0 {
        items.push(CleanupItem {
            id: format!("{}_cache", id_prefix),
            name: format!("{} Cache", name),
            path_display: format!("{} ({} perfil/perfis)", user_data_dir.to_string_lossy(), profiles.len()),
            size_bytes: total_size,
            file_count: total_count,
            default_selected: true,
        });
    }

    items
}

fn find_chromium_profiles(user_data_dir: &Path) -> Vec<PathBuf> {
    let mut profiles = Vec::new();
    let default_dir = user_data_dir.join("Default");
    if default_dir.exists() {
        profiles.push(default_dir);
    }
    // Profile 1, Profile 2, ...
    if let Ok(entries) = std::fs::read_dir(user_data_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("Profile ") && entry.path().is_dir() {
                profiles.push(entry.path());
            }
        }
    }
    profiles
}

fn scan_browsers() -> Option<CleanupCategory> {
    let local = local_app_data();
    let appdata = app_data();

    let mut items = Vec::new();

    // Chrome
    let chrome_dir = local.join(r"Google\Chrome\User Data");
    items.extend(scan_chromium_browser("chrome", "Chrome", &chrome_dir));

    // Edge
    let edge_dir = local.join(r"Microsoft\Edge\User Data");
    items.extend(scan_chromium_browser("edge", "Edge", &edge_dir));

    // Brave
    let brave_dir = local.join(r"BraveSoftware\Brave-Browser\User Data");
    items.extend(scan_chromium_browser("brave", "Brave", &brave_dir));

    // Opera
    let opera_dir = appdata.join(r"Opera Software\Opera Stable");
    if opera_dir.exists() {
        let opera_cache_dirs = vec![
            opera_dir.join(r"Cache\Cache_Data"),
            opera_dir.join("Code Cache"),
            opera_dir.join("GPUCache"),
        ];
        let (s, c) = scan_paths_stats(&opera_cache_dirs);
        if s > 0 {
            items.push(CleanupItem {
                id: "opera_cache".into(), name: "Opera Cache".into(),
                path_display: opera_dir.to_string_lossy().into(),
                size_bytes: s, file_count: c,
                default_selected: true,
            });
        }
    }

    // Opera GX
    let opera_gx_dir = appdata.join(r"Opera Software\Opera GX Stable");
    if opera_gx_dir.exists() {
        let gx_cache_dirs = vec![
            opera_gx_dir.join(r"Cache\Cache_Data"),
            opera_gx_dir.join("Code Cache"),
            opera_gx_dir.join("GPUCache"),
        ];
        let (s, c) = scan_paths_stats(&gx_cache_dirs);
        if s > 0 {
            items.push(CleanupItem {
                id: "opera_gx_cache".into(), name: "Opera GX Cache".into(),
                path_display: opera_gx_dir.to_string_lossy().into(),
                size_bytes: s, file_count: c,
                default_selected: true,
            });
        }
    }

    // Firefox
    let firefox_profiles = appdata.join(r"Mozilla\Firefox\Profiles");
    if firefox_profiles.exists() {
        let mut ff_size: u64 = 0;
        let mut ff_count: u32 = 0;
        if let Ok(entries) = std::fs::read_dir(&firefox_profiles) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let cache = entry.path().join(r"cache2\entries");
                    let (s, c) = scan_dir_stats(&cache);
                    ff_size += s;
                    ff_count += c;
                }
            }
        }
        if ff_size > 0 {
            items.push(CleanupItem {
                id: "firefox_cache".into(), name: "Firefox Cache".into(),
                path_display: firefox_profiles.to_string_lossy().into(),
                size_bytes: ff_size, file_count: ff_count,
                default_selected: true,
            });
        }
    }

    if items.is_empty() {
        return None;
    }

    let total_size: u64 = items.iter().map(|i| i.size_bytes).sum();
    let total_count: u32 = items.iter().map(|i| i.file_count).sum();

    Some(CleanupCategory {
        id: "browsers".into(),
        name: "Browsers".into(),
        description: "Cache de navegadores — nunca toca em senhas, bookmarks ou cookies".into(),
        risk: CleanupRisk::Safe,
        default_selected: true,
        items,
        total_size_bytes: total_size,
        total_file_count: total_count,
    })
}

fn scan_aplicativos() -> Option<CleanupCategory> {
    let local = local_app_data();
    let appdata = app_data();

    let mut items = Vec::new();

    // Spotify
    let spotify = local.join(r"Spotify\Storage");
    let (s, c) = scan_dir_stats(&spotify);
    if s > 0 {
        items.push(CleanupItem {
            id: "spotify_cache".into(), name: "Spotify Cache".into(),
            path_display: spotify.to_string_lossy().into(),
            size_bytes: s, file_count: c, default_selected: true,
        });
    }

    // Discord
    let discord_paths = vec![
        appdata.join(r"discord\Cache\Cache_Data"),
        appdata.join(r"discord\Code Cache"),
    ];
    let (s, c) = scan_paths_stats(&discord_paths);
    if s > 0 {
        items.push(CleanupItem {
            id: "discord_cache".into(), name: "Discord Cache".into(),
            path_display: "discord\\Cache + Code Cache".into(),
            size_bytes: s, file_count: c, default_selected: true,
        });
    }

    // Battle.net
    let bnet = local.join(r"Battle.net\Cache");
    let (s, c) = scan_dir_stats(&bnet);
    if s > 0 {
        items.push(CleanupItem {
            id: "battlenet_cache".into(), name: "Battle.net Cache".into(),
            path_display: bnet.to_string_lossy().into(),
            size_bytes: s, file_count: c, default_selected: true,
        });
    }

    // Epic Games (desmarcado — pode exigir re-login)
    let epic = local.join(r"EpicGamesLauncher\Saved\webcache");
    let (s, c) = scan_dir_stats(&epic);
    if s > 0 {
        items.push(CleanupItem {
            id: "epic_webcache".into(), name: "Epic Games Launcher".into(),
            path_display: epic.to_string_lossy().into(),
            size_bytes: s, file_count: c, default_selected: false,
        });
    }

    // Steam HTTP Cache (desmarcado)
    let steam_http = local.join(r"Steam\htmlcache\Cache");
    let (s, c) = scan_dir_stats(&steam_http);
    if s > 0 {
        items.push(CleanupItem {
            id: "steam_httpcache".into(), name: "Steam HTTP Cache".into(),
            path_display: steam_http.to_string_lossy().into(),
            size_bytes: s, file_count: c, default_selected: false,
        });
    }

    // Steam Shader Cache (desmarcado — stutter pesado)
    let steam_shader = local.join(r"Steam\shadercache");
    let (s2, c2) = scan_dir_stats(&steam_shader);
    // Também checar pasta steamapps
    let steam_shader2 = PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\shadercache");
    let (s3, c3) = scan_dir_stats(&steam_shader2);
    let ss = s2 + s3;
    let sc = c2 + c3;
    if ss > 0 {
        items.push(CleanupItem {
            id: "steam_shadercache".into(), name: "Steam Shader Cache".into(),
            path_display: "Steam\\shadercache (stutter pesado ao recompilar)".into(),
            size_bytes: ss, file_count: sc, default_selected: false,
        });
    }

    // npm cache (desmarcado)
    let npm = appdata.join("npm-cache");
    let (s, c) = scan_dir_stats(&npm);
    if s > 0 {
        items.push(CleanupItem {
            id: "npm_cache".into(), name: "npm Cache".into(),
            path_display: npm.to_string_lossy().into(),
            size_bytes: s, file_count: c, default_selected: false,
        });
    }

    // pip cache (desmarcado)
    let pip = local.join(r"pip\Cache");
    let (s, c) = scan_dir_stats(&pip);
    if s > 0 {
        items.push(CleanupItem {
            id: "pip_cache".into(), name: "pip Cache".into(),
            path_display: pip.to_string_lossy().into(),
            size_bytes: s, file_count: c, default_selected: false,
        });
    }

    // Yarn cache (desmarcado)
    let yarn = local.join(r"Yarn\Cache");
    let (s, c) = scan_dir_stats(&yarn);
    if s > 0 {
        items.push(CleanupItem {
            id: "yarn_cache".into(), name: "Yarn Cache".into(),
            path_display: yarn.to_string_lossy().into(),
            size_bytes: s, file_count: c, default_selected: false,
        });
    }

    // VS Code (desmarcado)
    let vscode_paths = vec![
        appdata.join(r"Code\CachedExtensions"),
        appdata.join(r"Code\Cache"),
    ];
    let (s, c) = scan_paths_stats(&vscode_paths);
    if s > 0 {
        items.push(CleanupItem {
            id: "vscode_cache".into(), name: "VS Code Cache".into(),
            path_display: "Code\\CachedExtensions + Cache".into(),
            size_bytes: s, file_count: c, default_selected: false,
        });
    }

    // Adobe (desmarcado)
    let adobe = local.join(r"Adobe\Common\Media Cache Files");
    let (s, c) = scan_dir_stats(&adobe);
    if s > 0 {
        items.push(CleanupItem {
            id: "adobe_cache".into(), name: "Adobe Media Cache".into(),
            path_display: adobe.to_string_lossy().into(),
            size_bytes: s, file_count: c, default_selected: false,
        });
    }

    // OBS logs (desmarcado)
    let obs = appdata.join(r"obs-studio\logs");
    let (s, c) = scan_dir_stats(&obs);
    if s > 0 {
        items.push(CleanupItem {
            id: "obs_logs".into(), name: "OBS Logs".into(),
            path_display: obs.to_string_lossy().into(),
            size_bytes: s, file_count: c, default_selected: false,
        });
    }

    if items.is_empty() {
        return None;
    }

    let total_size: u64 = items.iter().map(|i| i.size_bytes).sum();
    let total_count: u32 = items.iter().map(|i| i.file_count).sum();

    Some(CleanupCategory {
        id: "aplicativos".into(),
        name: "Aplicativos".into(),
        description: "Caches de apps instalados detectados no sistema".into(),
        risk: CleanupRisk::Moderate,
        default_selected: false,
        items,
        total_size_bytes: total_size,
        total_file_count: total_count,
    })
}

fn scan_avancado() -> CleanupCategory {
    let mut items = Vec::new();

    // Windows.old
    let win_old = PathBuf::from(r"C:\Windows.old");
    if win_old.exists() {
        let (s, c) = scan_dir_stats(&win_old);
        let age_desc = match std::fs::metadata(&win_old).and_then(|m| m.created()) {
            Ok(created) => {
                let age_days = created.elapsed().map(|d| d.as_secs() / 86400).unwrap_or(0);
                format!("Windows.old ({} dias)", age_days)
            }
            Err(_) => "Windows.old".into(),
        };
        items.push(CleanupItem {
            id: "windows_old".into(), name: age_desc,
            path_display: r"C:\Windows.old (irreversível)".into(),
            size_bytes: s, file_count: c, default_selected: false,
        });
    }

    // WinSxS Cleanup
    items.push(CleanupItem {
        id: "winsxs_cleanup".into(),
        name: "WinSxS Cleanup (DISM)".into(),
        path_display: "DISM /Online /Cleanup-Image /StartComponentCleanup".into(),
        size_bytes: 0,
        file_count: 0,
        default_selected: false,
    });

    let total_size: u64 = items.iter().map(|i| i.size_bytes).sum();
    let total_count: u32 = items.iter().map(|i| i.file_count).sum();

    CleanupCategory {
        id: "avancado".into(),
        name: "Avançado".into(),
        description: "Operações pesadas e irreversíveis — use com cuidado".into(),
        risk: CleanupRisk::Caution,
        default_selected: false,
        items,
        total_size_bytes: total_size,
        total_file_count: total_count,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Comandos Tauri
// ═══════════════════════════════════════════════════════════════════════════════

/// Escaneia o sistema e retorna categorias de limpeza com tamanhos reais.
#[tauri::command]
pub async fn scan_cleanup() -> Result<CleanupScanResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let mut categories: Vec<CleanupCategory> = Vec::new();

        categories.push(scan_sistema_windows());
        categories.push(scan_gpu_shader_cache());

        if let Some(cat) = scan_browsers() {
            categories.push(cat);
        }
        if let Some(cat) = scan_aplicativos() {
            categories.push(cat);
        }

        categories.push(scan_avancado());

        // Remove categorias vazias (exceto avançado que sempre mostra WinSxS)
        categories.retain(|c| !c.items.is_empty());

        let total_size: u64 = categories.iter().map(|c| c.total_size_bytes).sum();
        let total_count: u32 = categories.iter().map(|c| c.total_file_count).sum();

        Ok(CleanupScanResult {
            categories,
            total_size_bytes: total_size,
            total_file_count: total_count,
            scan_duration_seconds: start.elapsed().as_secs(),
        })
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

/// Executa a limpeza dos itens selecionados com progresso em tempo real.
///
/// Recebe IDs de items (não de categorias). Emite `CleanupProgressEvent`
/// no canal `cleanup_progress`.
#[tauri::command]
pub async fn execute_cleanup(
    app: tauri::AppHandle,
    item_ids: Vec<String>,
) -> Result<CleanupResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let total_items = item_ids.len();
        let mut item_results: Vec<CleanupItemResult> = Vec::new();
        let mut all_locked_paths: Vec<String> = Vec::new();
        let mut global_freed: u64 = 0;

        for (idx, item_id) in item_ids.iter().enumerate() {
            let item_name = get_item_display_name(item_id);

            // Emitir progresso
            let progress = (idx as f64 / total_items as f64) * 100.0;
            let _ = app.emit("cleanup_progress", CleanupProgressEvent {
                current_category: get_item_category(item_id).to_string(),
                current_item: item_name.clone(),
                progress_percent: progress,
                freed_bytes_so_far: global_freed,
                message: format!("Limpando: {}", item_name),
            });

            let result = execute_single_item(&app, item_id);
            global_freed += result.freed_bytes;
            all_locked_paths.extend(result.locked_paths.clone());

            item_results.push(CleanupItemResult {
                id: item_id.clone(),
                name: item_name,
                freed_bytes: result.freed_bytes,
                files_removed: result.files_removed,
                files_skipped: result.files_skipped,
                errors: result.errors,
            });
        }

        // Progresso 100%
        let _ = app.emit("cleanup_progress", CleanupProgressEvent {
            current_category: String::new(),
            current_item: "Concluído".into(),
            progress_percent: 100.0,
            freed_bytes_so_far: global_freed,
            message: "Limpeza concluída".into(),
        });

        let total_removed: u32 = item_results.iter().map(|r| r.files_removed).sum();
        let total_skipped: u32 = item_results.iter().map(|r| r.files_skipped).sum();
        let locking_procs = aggregate_locking_processes(&all_locked_paths);

        Ok(CleanupResult {
            total_freed_bytes: global_freed,
            total_files_removed: total_removed,
            total_files_skipped: total_skipped,
            duration_seconds: start.elapsed().as_secs(),
            item_results,
            locking_processes: locking_procs,
        })
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

// ─── Execução por item ───────────────────────────────────────────────────────

struct ItemExecResult {
    freed_bytes: u64,
    files_removed: u32,
    files_skipped: u32,
    errors: Vec<String>,
    locked_paths: Vec<String>,
}

fn execute_single_item(_app: &tauri::AppHandle, item_id: &str) -> ItemExecResult {
    match item_id {
        // ── Sistema Windows ──
        "temp_user" => delete_item_dirs(&[temp_dir()]),
        "temp_windows" => delete_item_dirs(&[PathBuf::from(r"C:\Windows\Temp")]),
        "wer_reports" => {
            let local = local_app_data();
            delete_item_dirs(&[
                local.join(r"Microsoft\Windows\WER\ReportArchive"),
                local.join(r"Microsoft\Windows\WER\ReportQueue"),
                PathBuf::from(r"C:\ProgramData\Microsoft\Windows\WER"),
            ])
        }
        "wu_cache" => execute_wu_cache_cleanup(),
        "delivery_optim" => execute_delivery_optim_cleanup(),
        "thumbcache" => execute_thumbcache_cleanup(),
        "memory_dumps" => {
            let mut result = delete_single_file(&PathBuf::from(r"C:\Windows\MEMORY.DMP"));
            let dir_result = delete_item_dirs(&[PathBuf::from(r"C:\Windows\Minidump")]);
            result.freed_bytes += dir_result.freed_bytes;
            result.files_removed += dir_result.files_removed;
            result.files_skipped += dir_result.files_skipped;
            result.errors.extend(dir_result.errors);
            result.locked_paths.extend(dir_result.locked_paths);
            result
        }
        "cbs_logs" => delete_item_dirs(&[
            PathBuf::from(r"C:\Windows\Logs\CBS"),
            PathBuf::from(r"C:\Windows\Logs\DISM"),
            PathBuf::from(r"C:\Windows\Logs\WindowsUpdate"),
        ]),
        "recycle_bin" => execute_recycle_bin_cleanup(),
        "downloaded_programs" => delete_item_dirs(&[PathBuf::from(r"C:\Windows\Downloaded Program Files")]),

        // ── GPU Shader Cache ──
        "nvidia_dxcache" => delete_item_dirs(&[local_app_data().join(r"NVIDIA\DXCache")]),
        "nvidia_glcache" => delete_item_dirs(&[local_app_data().join(r"NVIDIA\GLCache")]),
        "nvidia_computecache" => delete_item_dirs(&[app_data().join(r"NVIDIA\ComputeCache")]),
        "amd_dxcache" => delete_item_dirs(&[local_app_data().join(r"AMD\DxCache")]),
        "amd_dxccache" => delete_item_dirs(&[local_app_data().join(r"AMD\DxcCache")]),
        "amd_glcache" => delete_item_dirs(&[local_app_data().join(r"AMD\GLCache")]),
        "amd_vkcache" => delete_item_dirs(&[local_app_data().join(r"AMD\VkCache")]),
        "intel_shadercache" => delete_item_dirs(&[local_app_data().join(r"Intel\ShaderCache")]),
        "dx_d3dscache" => delete_item_dirs(&[local_app_data().join("D3DSCache")]),

        // ── Browsers ──
        "chrome_cache" => delete_chromium_cache(&local_app_data().join(r"Google\Chrome\User Data")),
        "edge_cache" => delete_chromium_cache(&local_app_data().join(r"Microsoft\Edge\User Data")),
        "brave_cache" => delete_chromium_cache(&local_app_data().join(r"BraveSoftware\Brave-Browser\User Data")),
        "opera_cache" => {
            let dir = app_data().join(r"Opera Software\Opera Stable");
            delete_item_dirs(&[
                dir.join(r"Cache\Cache_Data"),
                dir.join("Code Cache"),
                dir.join("GPUCache"),
            ])
        }
        "opera_gx_cache" => {
            let dir = app_data().join(r"Opera Software\Opera GX Stable");
            delete_item_dirs(&[
                dir.join(r"Cache\Cache_Data"),
                dir.join("Code Cache"),
                dir.join("GPUCache"),
            ])
        }
        "firefox_cache" => execute_firefox_cache_cleanup(),

        // ── Aplicativos ──
        "spotify_cache" => delete_item_dirs(&[local_app_data().join(r"Spotify\Storage")]),
        "discord_cache" => delete_item_dirs(&[
            app_data().join(r"discord\Cache\Cache_Data"),
            app_data().join(r"discord\Code Cache"),
        ]),
        "battlenet_cache" => delete_item_dirs(&[local_app_data().join(r"Battle.net\Cache")]),
        "epic_webcache" => delete_item_dirs(&[local_app_data().join(r"EpicGamesLauncher\Saved\webcache")]),
        "steam_httpcache" => delete_item_dirs(&[local_app_data().join(r"Steam\htmlcache\Cache")]),
        "steam_shadercache" => delete_item_dirs(&[
            local_app_data().join(r"Steam\shadercache"),
            PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\shadercache"),
        ]),
        "npm_cache" => delete_item_dirs(&[app_data().join("npm-cache")]),
        "pip_cache" => delete_item_dirs(&[local_app_data().join(r"pip\Cache")]),
        "yarn_cache" => delete_item_dirs(&[local_app_data().join(r"Yarn\Cache")]),
        "vscode_cache" => delete_item_dirs(&[
            app_data().join(r"Code\CachedExtensions"),
            app_data().join(r"Code\Cache"),
        ]),
        "adobe_cache" => delete_item_dirs(&[local_app_data().join(r"Adobe\Common\Media Cache Files")]),
        "obs_logs" => delete_item_dirs(&[app_data().join(r"obs-studio\logs")]),

        // ── Avançado ──
        "windows_old" => execute_windows_old_cleanup(),
        "winsxs_cleanup" => execute_winsxs_cleanup(_app),

        _ => ItemExecResult {
            freed_bytes: 0, files_removed: 0, files_skipped: 0,
            errors: vec![format!("Item desconhecido: {}", item_id)],
            locked_paths: Vec::new(),
        },
    }
}

/// Limpeza padrão: delete conteúdo de diretórios.
fn delete_item_dirs(paths: &[PathBuf]) -> ItemExecResult {
    let mut freed: u64 = 0;
    let r = delete_paths_contents(paths, &mut freed);
    ItemExecResult {
        freed_bytes: freed,
        files_removed: r.files_removed,
        files_skipped: r.files_skipped,
        errors: r.errors,
        locked_paths: r.locked_paths,
    }
}

/// Deleta um único arquivo.
fn delete_single_file(path: &Path) -> ItemExecResult {
    if !path.exists() {
        return ItemExecResult {
            freed_bytes: 0, files_removed: 0, files_skipped: 0,
            errors: Vec::new(), locked_paths: Vec::new(),
        };
    }
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    match std::fs::remove_file(path) {
        Ok(()) => ItemExecResult {
            freed_bytes: size, files_removed: 1, files_skipped: 0,
            errors: Vec::new(), locked_paths: Vec::new(),
        },
        Err(e) => ItemExecResult {
            freed_bytes: 0, files_removed: 0, files_skipped: 1,
            errors: vec![format!("{}: {}", path.display(), e)],
            locked_paths: vec![path.to_string_lossy().into()],
        },
    }
}

// ─── Execuções especiais ─────────────────────────────────────────────────────

/// Windows Update Cache — para o serviço wuauserv antes de limpar.
fn execute_wu_cache_cleanup() -> ItemExecResult {
    let _ = run_powershell("Stop-Service wuauserv -Force -ErrorAction SilentlyContinue");
    let result = delete_item_dirs(&[PathBuf::from(r"C:\Windows\SoftwareDistribution\Download")]);
    let _ = run_powershell("Start-Service wuauserv -ErrorAction SilentlyContinue");
    result
}

/// Delivery Optimization — usa cmdlet PowerShell dedicado.
fn execute_delivery_optim_cleanup() -> ItemExecResult {
    // Medir tamanho antes
    let cache_path = PathBuf::from(
        r"C:\Windows\ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache"
    );
    let (size_before, _) = scan_dir_stats(&cache_path);

    match run_powershell("[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Clear-DeliveryOptimizationCache -Force -ErrorAction SilentlyContinue") {
        Ok(_) => {
            let (size_after, _) = scan_dir_stats(&cache_path);
            let freed = size_before.saturating_sub(size_after);
            ItemExecResult {
                freed_bytes: freed, files_removed: 1, files_skipped: 0,
                errors: Vec::new(), locked_paths: Vec::new(),
            }
        }
        Err(e) => ItemExecResult {
            freed_bytes: 0, files_removed: 0, files_skipped: 0,
            errors: vec![format!("Clear-DeliveryOptimizationCache: {}", e)],
            locked_paths: Vec::new(),
        },
    }
}

/// Thumbnail Cache — deleta thumbcache_*.db e iconcache_*.db.
fn execute_thumbcache_cleanup() -> ItemExecResult {
    let explorer_dir = local_app_data().join(r"Microsoft\Windows\Explorer");
    let mut freed: u64 = 0;
    let mut removed: u32 = 0;
    let mut skipped: u32 = 0;
    let mut errors = Vec::new();
    let mut locked = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&explorer_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let is_cache = (name.starts_with("thumbcache_") || name.starts_with("iconcache_"))
                && name.ends_with(".db");
            if !is_cache {
                continue;
            }
            let path = entry.path();
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    freed += size;
                    removed += 1;
                }
                Err(e) => {
                    skipped += 1;
                    let raw = e.raw_os_error();
                    if raw == Some(32) || raw == Some(5) {
                        locked.push(path.to_string_lossy().to_string());
                    }
                    errors.push(format!("{}: {}", name, e));
                }
            }
        }
    }

    ItemExecResult { freed_bytes: freed, files_removed: removed, files_skipped: skipped, errors, locked_paths: locked }
}

/// Lixeira — usa PowerShell Clear-RecycleBin.
fn execute_recycle_bin_cleanup() -> ItemExecResult {
    let size_before = get_recycle_bin_size();
    match run_powershell("[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Clear-RecycleBin -Force -ErrorAction SilentlyContinue") {
        Ok(_) => ItemExecResult {
            freed_bytes: size_before, files_removed: 1, files_skipped: 0,
            errors: Vec::new(), locked_paths: Vec::new(),
        },
        Err(e) => ItemExecResult {
            freed_bytes: 0, files_removed: 0, files_skipped: 0,
            errors: vec![format!("Clear-RecycleBin: {}", e)],
            locked_paths: Vec::new(),
        },
    }
}

/// Limpa cache de browser Chromium (múltiplos perfis).
fn delete_chromium_cache(user_data_dir: &Path) -> ItemExecResult {
    let profiles = find_chromium_profiles(user_data_dir);
    let mut all_paths: Vec<PathBuf> = Vec::new();
    for profile in &profiles {
        all_paths.push(profile.join(r"Cache\Cache_Data"));
        all_paths.push(profile.join("Code Cache"));
        all_paths.push(profile.join("GPUCache"));
        all_paths.push(profile.join(r"Service Worker\CacheStorage"));
    }
    delete_item_dirs(&all_paths)
}

/// Limpa cache do Firefox (múltiplos perfis).
fn execute_firefox_cache_cleanup() -> ItemExecResult {
    let profiles_dir = app_data().join(r"Mozilla\Firefox\Profiles");
    let mut cache_paths: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                cache_paths.push(entry.path().join(r"cache2\entries"));
            }
        }
    }
    delete_item_dirs(&cache_paths)
}

/// Remove Windows.old via PowerShell com força.
fn execute_windows_old_cleanup() -> ItemExecResult {
    let win_old = PathBuf::from(r"C:\Windows.old");
    let (size_before, _) = scan_dir_stats(&win_old);

    match run_powershell("Remove-Item -Path 'C:\\Windows.old' -Recurse -Force -ErrorAction SilentlyContinue") {
        Ok(_) => {
            let still_exists = win_old.exists();
            let freed = if still_exists {
                let (size_after, _) = scan_dir_stats(&win_old);
                size_before.saturating_sub(size_after)
            } else {
                size_before
            };
            ItemExecResult {
                freed_bytes: freed, files_removed: 1, files_skipped: 0,
                errors: Vec::new(), locked_paths: Vec::new(),
            }
        }
        Err(e) => ItemExecResult {
            freed_bytes: 0, files_removed: 0, files_skipped: 0,
            errors: vec![format!("Windows.old: {}", e)],
            locked_paths: Vec::new(),
        },
    }
}

/// WinSxS Cleanup via DISM StartComponentCleanup.
fn execute_winsxs_cleanup(_app: &tauri::AppHandle) -> ItemExecResult {
    let script = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; dism.exe /Online /Cleanup-Image /StartComponentCleanup";
    match run_powershell(script) {
        Ok(out) => {
            let success = out.exit_code == 0;
            ItemExecResult {
                freed_bytes: 0, // DISM não reporta bytes exatos facilmente
                files_removed: if success { 1 } else { 0 },
                files_skipped: 0,
                errors: if success { Vec::new() } else { vec![out.stderr.clone()] },
                locked_paths: Vec::new(),
            }
        }
        Err(e) => ItemExecResult {
            freed_bytes: 0, files_removed: 0, files_skipped: 0,
            errors: vec![format!("DISM WinSxS: {}", e)],
            locked_paths: Vec::new(),
        },
    }
}

// ─── Mapeamento de nomes e categorias ────────────────────────────────────────

fn get_item_display_name(item_id: &str) -> String {
    match item_id {
        "temp_user" => "Temp do Usuário",
        "temp_windows" => "Windows Temp",
        "wer_reports" => "Windows Error Reports",
        "wu_cache" => "Windows Update Cache",
        "delivery_optim" => "Delivery Optimization Cache",
        "thumbcache" => "Thumbnail Cache",
        "memory_dumps" => "Memory Dumps",
        "cbs_logs" => "CBS/DISM/Upgrade Logs",
        "recycle_bin" => "Lixeira",
        "downloaded_programs" => "Downloaded Program Files",
        "nvidia_dxcache" => "NVIDIA DXCache",
        "nvidia_glcache" => "NVIDIA GLCache",
        "nvidia_computecache" => "NVIDIA ComputeCache",
        "amd_dxcache" => "AMD DxCache",
        "amd_dxccache" => "AMD DxcCache",
        "amd_glcache" => "AMD GLCache",
        "amd_vkcache" => "AMD VkCache",
        "intel_shadercache" => "Intel ShaderCache",
        "dx_d3dscache" => "DirectX D3DSCache",
        "chrome_cache" => "Chrome Cache",
        "edge_cache" => "Edge Cache",
        "brave_cache" => "Brave Cache",
        "opera_cache" => "Opera Cache",
        "opera_gx_cache" => "Opera GX Cache",
        "firefox_cache" => "Firefox Cache",
        "spotify_cache" => "Spotify Cache",
        "discord_cache" => "Discord Cache",
        "battlenet_cache" => "Battle.net Cache",
        "epic_webcache" => "Epic Games Launcher",
        "steam_httpcache" => "Steam HTTP Cache",
        "steam_shadercache" => "Steam Shader Cache",
        "npm_cache" => "npm Cache",
        "pip_cache" => "pip Cache",
        "yarn_cache" => "Yarn Cache",
        "vscode_cache" => "VS Code Cache",
        "adobe_cache" => "Adobe Media Cache",
        "obs_logs" => "OBS Logs",
        "windows_old" => "Windows.old",
        "winsxs_cleanup" => "WinSxS Cleanup (DISM)",
        _ => item_id,
    }.to_string()
}

fn get_item_category(item_id: &str) -> &'static str {
    match item_id {
        "temp_user" | "temp_windows" | "wer_reports" | "wu_cache" | "delivery_optim"
        | "thumbcache" | "memory_dumps" | "cbs_logs" | "recycle_bin" | "downloaded_programs" => {
            "Sistema Windows"
        }
        s if s.starts_with("nvidia_") || s.starts_with("amd_") || s.starts_with("intel_")
            || s == "dx_d3dscache" => {
            "GPU Shader Cache"
        }
        "chrome_cache" | "edge_cache" | "brave_cache" | "opera_cache" | "opera_gx_cache"
        | "firefox_cache" => "Browsers",
        "spotify_cache" | "discord_cache" | "battlenet_cache" | "epic_webcache"
        | "steam_httpcache" | "steam_shadercache" | "npm_cache" | "pip_cache" | "yarn_cache"
        | "vscode_cache" | "adobe_cache" | "obs_logs" => "Aplicativos",
        "windows_old" | "winsxs_cleanup" => "Avançado",
        _ => "Outros",
    }
}
