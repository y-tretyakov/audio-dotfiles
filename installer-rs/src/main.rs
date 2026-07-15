use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{anyhow, Context};
use chrono::Local;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use ratatui::{
    backend::{CrosstermBackend, Backend},
    Terminal,
};
use tar::{Archive, Builder};

mod ui;

// ============= Constants =============
const REPO_ROOT: &str = "/home/code_warlord/projects/audio-dotfiles";

// ============= Types =============
#[derive(Debug, Clone, PartialEq)]
pub enum AppStep {
    Idle,
    BackingUp,
    DeployingPresets,
    DeployingIRs,
    DeployingPipeWire,
    DeployingNiri,
    RestartingServices,
    Done,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Deploy,
    Rollback,
    DryRun,
}

#[derive(Debug, Clone)]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Warning(String),
    Error(String),
}

pub struct AppState {
    pub mode: AppMode,
    pub current_step: AppStep,
    pub progress: f32,
    pub step_statuses: Vec<(&'static str, StepStatus)>,
    pub log_messages: Vec<String>,
    pub backup_path: Option<String>,
    pub should_quit: bool,
}

impl AppState {
    fn new(mode: AppMode) -> Self {
        Self {
            mode,
            current_step: AppStep::Idle,
            progress: 0.0,
            step_statuses: vec![
                ("Backup", StepStatus::Pending),
                ("EasyEffects Presets", StepStatus::Pending),
                ("EasyEffects IRs", StepStatus::Pending),
                ("PipeWire Config", StepStatus::Pending),
                ("niri Autostart", StepStatus::Pending),
                ("Restart Services", StepStatus::Pending),
            ],
            log_messages: Vec::new(),
            backup_path: None,
            should_quit: false,
        }
    }
}

// ============= CLI =============
#[derive(Parser)]
#[command(name = "audio-manager", about = "Deploy audio dotfiles")]
struct Cli {
    #[arg(long)]
    dry_run: bool,
    #[arg(long, short)]
    rollback: bool,
}

// ============= Helpers =============
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(rest)
    } else {
        PathBuf::from(path)
    }
}

fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

fn symlink_file(src: &Path, dst: &Path, state: &mut AppState) -> anyhow::Result<()> {
    if let AppMode::DryRun = state.mode {
        state
            .log_messages
            .push(format!("Would symlink: {} -> {}", dst.display(), src.display()));
        return Ok(());
    }

    if dst.exists() || dst.is_symlink() {
        if let Ok(target) = fs::read_link(dst) {
            if target == src {
                return Ok(());
            }
        }
        if dst.is_symlink() {
            fs::remove_file(dst)?;
        } else {
            state.log_messages.push(format!(
                "Warning: {} exists as a regular file, skipping",
                dst.display()
            ));
            return Ok(());
        }
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    std::os::unix::fs::symlink(src, dst)?;
    state
        .log_messages
        .push(format!("Linked: {} -> {}", dst.display(), src.display()));
    Ok(())
}

// ============= Step Executors =============
fn execute_backup(state: &mut AppState) -> anyhow::Result<String> {
    state.step_statuses[0].1 = StepStatus::Running;
    state.current_step = AppStep::BackingUp;

    let now = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_path = expand_tilde(&format!("~/.config/audio_backup_{}.tar.gz", now));

    let home = home_dir();
    let dirs_to_backup = [
        (
            home.join(".local/share/easyeffects/output"),
            ".local/share/easyeffects/output",
        ),
        (
            home.join(".local/share/easyeffects/irs"),
            ".local/share/easyeffects/irs",
        ),
        (
            home.join(".config/pipewire/pipewire.conf.d"),
            ".config/pipewire/pipewire.conf.d",
        ),
    ];
    let files_to_backup = [(home.join(".config/niri/cfg/autostart.kdl"), ".config/niri/cfg/autostart.kdl")];

    if let AppMode::DryRun = state.mode {
        state
            .log_messages
            .push(format!("Would create backup: {}", backup_path.display()));
        for (src, _) in &dirs_to_backup {
            if src.exists() {
                state
                    .log_messages
                    .push(format!("  Would archive directory: {}", src.display()));
            }
        }
        for (src, _) in &files_to_backup {
            if src.exists() {
                state
                    .log_messages
                    .push(format!("  Would archive file: {}", src.display()));
            }
        }
        state.progress = 0.2;
        return Ok(backup_path.to_string_lossy().to_string());
    }

    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = File::create(&backup_path)
        .with_context(|| format!("Failed to create backup file: {}", backup_path.display()))?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);

    for (src_path, archive_path) in &dirs_to_backup {
        if src_path.exists() {
            builder
                .append_dir_all(archive_path, src_path)
                .with_context(|| format!("Failed to archive directory: {}", src_path.display()))?;
        }
    }

    for (src_path, archive_path) in &files_to_backup {
        if src_path.exists() {
            let contents = fs::read(src_path)
                .with_context(|| format!("Failed to read file: {}", src_path.display()))?;
            let mut header = tar::Header::new_gnu();
            header.set_path(archive_path)?;
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, &contents[..])?;
        }
    }

    builder.finish()?;

    state.progress = 0.2;
    state
        .log_messages
        .push(format!("Created backup: {}", backup_path.display()));
    state.backup_path = Some(backup_path.to_string_lossy().to_string());

    Ok(backup_path.to_string_lossy().to_string())
}

fn deploy_presets(state: &mut AppState) -> anyhow::Result<()> {
    state.step_statuses[1].1 = StepStatus::Running;
    state.current_step = AppStep::DeployingPresets;

    let repo_dir = PathBuf::from(REPO_ROOT).join("easyeffects/output");
    let target_dir = home_dir().join(".local/share/easyeffects/output");

    if !repo_dir.exists() {
        return Err(anyhow!(
            "Repo presets directory not found: {}",
            repo_dir.display()
        ));
    }

    fs::create_dir_all(&target_dir)?;

    let entries = fs::read_dir(&repo_dir)?;
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() && entry.path().extension().is_some_and(|e| e == "json") {
            let src = entry.path();
            let dst = target_dir.join(entry.file_name());
            symlink_file(&src, &dst, state)?;
        }
    }

    state.step_statuses[1].1 = StepStatus::Done;
    state.progress = 0.4;
    Ok(())
}

fn deploy_irs(state: &mut AppState) -> anyhow::Result<()> {
    state.step_statuses[2].1 = StepStatus::Running;
    state.current_step = AppStep::DeployingIRs;

    let repo_dir = PathBuf::from(REPO_ROOT).join("easyeffects/irs");
    let target_dir = home_dir().join(".local/share/easyeffects/irs");

    if !repo_dir.exists() {
        return Err(anyhow!("Repo IRs directory not found: {}", repo_dir.display()));
    }

    fs::create_dir_all(&target_dir)?;

    let entries = fs::read_dir(&repo_dir)?;
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let src = entry.path();
            let dst = target_dir.join(entry.file_name());
            symlink_file(&src, &dst, state)?;
        }
    }

    state.step_statuses[2].1 = StepStatus::Done;
    state.progress = 0.6;
    Ok(())
}

fn deploy_pipewire(state: &mut AppState) -> anyhow::Result<()> {
    state.step_statuses[3].1 = StepStatus::Running;
    state.current_step = AppStep::DeployingPipeWire;

    let src = PathBuf::from(REPO_ROOT).join("pipewire/pipewire.conf.d/10-quantum.conf");
    let dst = home_dir().join(".config/pipewire/pipewire.conf.d/10-quantum.conf");

    if !src.exists() {
        return Err(anyhow!("PipeWire config not found: {}", src.display()));
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    symlink_file(&src, &dst, state)?;

    state.step_statuses[3].1 = StepStatus::Done;
    state.progress = 0.7;
    Ok(())
}

fn deploy_niri(state: &mut AppState) -> anyhow::Result<()> {
    state.step_statuses[4].1 = StepStatus::Running;
    state.current_step = AppStep::DeployingNiri;

    let autostart_path = home_dir().join(".config/niri/cfg/autostart.kdl");
    let line_to_add = r#"run-on-spawn "easyeffects" "--gapplication-service""#;

    if let AppMode::DryRun = state.mode {
        if autostart_path.exists() {
            let content = fs::read_to_string(&autostart_path)?;
            if !content.contains("easyeffects") {
                state.log_messages.push(format!(
                    "Would add to {}: {}",
                    autostart_path.display(),
                    line_to_add
                ));
            } else {
                state
                    .log_messages
                    .push("niri autostart already configured".to_string());
            }
        } else {
            state.log_messages.push(format!(
                "Would create {} with easyeffects entry",
                autostart_path.display()
            ));
        }
        state.step_statuses[4].1 = StepStatus::Done;
        state.progress = 0.8;
        return Ok(());
    }

    if autostart_path.exists() {
        let content = fs::read_to_string(&autostart_path)?;
        if content.contains("easyeffects") {
            state
                .log_messages
                .push("niri autostart already configured, skipping".to_string());
            state.step_statuses[4].1 = StepStatus::Done;
            state.progress = 0.8;
            return Ok(());
        }
        let mut file = fs::OpenOptions::new().append(true).open(&autostart_path)?;
        writeln!(file, "{}", line_to_add)?;
        state
            .log_messages
            .push("Added easyeffects to niri autostart".to_string());
    } else {
        if let Some(parent) = autostart_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(&autostart_path)?;
        writeln!(file, "{}", line_to_add)?;
        state
            .log_messages
            .push("Created niri autostart with easyeffects entry".to_string());
    }

    state.step_statuses[4].1 = StepStatus::Done;
    state.progress = 0.8;
    Ok(())
}

fn restart_services(state: &mut AppState) -> anyhow::Result<()> {
    state.step_statuses[5].1 = StepStatus::Running;
    state.current_step = AppStep::RestartingServices;

    if let AppMode::DryRun = state.mode {
        state
            .log_messages
            .push("Would restart pipewire, pipewire-pulse, wireplumber".to_string());
        state.step_statuses[5].1 = StepStatus::Done;
        state.progress = 1.0;
        return Ok(());
    }

    let output = std::process::Command::new("systemctl")
        .args(["--user", "restart", "pipewire", "pipewire-pulse", "wireplumber"])
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                state
                    .log_messages
                    .push("Services restarted successfully".to_string());
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                state
                    .log_messages
                    .push(format!("Warning: service restart had issues: {}", stderr));
            }
        }
        Err(e) => {
            state
                .log_messages
                .push(format!("Warning: could not restart services: {}", e));
        }
    }

    state.step_statuses[5].1 = StepStatus::Done;
    state.progress = 1.0;
    Ok(())
}

fn rollback(state: &mut AppState) -> anyhow::Result<()> {
    state.step_statuses[0].1 = StepStatus::Running;
    state.current_step = AppStep::BackingUp;

    let backup_dir = expand_tilde("~/.config");

    let mut entries: Vec<_> = fs::read_dir(&backup_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("audio_backup_") && name.ends_with(".tar.gz")
        })
        .collect();

    entries.sort_by_key(|e| e.file_name());

    let latest = entries
        .last()
        .ok_or_else(|| anyhow!("No backup found in {}", backup_dir.display()))?;
    let backup_path = latest.path();

    state
        .log_messages
        .push(format!("Restoring from backup: {}", backup_path.display()));

    let temp_dir = PathBuf::from("/tmp").join(format!("audio_rollback_{}", std::process::id()));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let file = File::open(&backup_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    archive.unpack(&temp_dir)?;

    state.progress = 0.3;
    state.step_statuses[0].1 = StepStatus::Done;
    state.step_statuses[1].1 = StepStatus::Running;

    let home = home_dir();
    let restore_pairs = [
        (
            temp_dir.join(".local/share/easyeffects/output"),
            home.join(".local/share/easyeffects/output"),
        ),
        (
            temp_dir.join(".local/share/easyeffects/irs"),
            home.join(".local/share/easyeffects/irs"),
        ),
        (
            temp_dir.join(".config/pipewire/pipewire.conf.d"),
            home.join(".config/pipewire/pipewire.conf.d"),
        ),
        (
            temp_dir.join(".config/niri/cfg/autostart.kdl"),
            home.join(".config/niri/cfg/autostart.kdl"),
        ),
    ];

    for (src, dst) in &restore_pairs {
        if src.exists() {
            if dst.exists() {
                if dst.is_dir() {
                    fs::remove_dir_all(dst)?;
                } else {
                    fs::remove_file(dst)?;
                }
            }
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(src, dst)?;
            state
                .log_messages
                .push(format!("Restored: {} -> {}", src.display(), dst.display()));
        }
    }

    state.progress = 0.7;
    state.step_statuses[1].1 = StepStatus::Done;

    restart_services(state)?;

    state.current_step = AppStep::Done;
    state.log_messages.push("Rollback complete".to_string());

    let _ = fs::remove_dir_all(&temp_dir);

    Ok(())
}

// ============= State Machine =============
fn start_deploy(app: &mut AppState) {
    if app.current_step != AppStep::Idle {
        return;
    }
    app.current_step = AppStep::BackingUp;
}

fn start_rollback(app: &mut AppState) {
    if app.current_step != AppStep::Idle {
        return;
    }
    app.mode = AppMode::Rollback;
    app.current_step = AppStep::BackingUp;
}

fn update(app: &mut AppState) {
    match app.current_step.clone() {
        AppStep::Idle => {}
        AppStep::BackingUp => {
            if app.mode == AppMode::Rollback {
                match rollback(app) {
                    Ok(()) => app.current_step = AppStep::Done,
                    Err(e) => app.current_step = AppStep::Error(e.to_string()),
                }
            } else {
                match execute_backup(app) {
                    Ok(_) => app.current_step = AppStep::DeployingPresets,
                    Err(e) => app.current_step = AppStep::Error(e.to_string()),
                }
            }
        }
        AppStep::DeployingPresets => match deploy_presets(app) {
            Ok(()) => app.current_step = AppStep::DeployingIRs,
            Err(e) => app.current_step = AppStep::Error(e.to_string()),
        },
        AppStep::DeployingIRs => match deploy_irs(app) {
            Ok(()) => app.current_step = AppStep::DeployingPipeWire,
            Err(e) => app.current_step = AppStep::Error(e.to_string()),
        },
        AppStep::DeployingPipeWire => match deploy_pipewire(app) {
            Ok(()) => app.current_step = AppStep::DeployingNiri,
            Err(e) => app.current_step = AppStep::Error(e.to_string()),
        },
        AppStep::DeployingNiri => match deploy_niri(app) {
            Ok(()) => app.current_step = AppStep::RestartingServices,
            Err(e) => app.current_step = AppStep::Error(e.to_string()),
        },
        AppStep::RestartingServices => match restart_services(app) {
            Ok(()) => app.current_step = AppStep::Done,
            Err(e) => app.current_step = AppStep::Error(e.to_string()),
        },
        AppStep::Done | AppStep::Error(_) => {}
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: AppState) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                        KeyCode::Char('d') => {
                            if app.mode != AppMode::Rollback {
                                start_deploy(&mut app);
                            }
                        }
                        KeyCode::Char('r') => start_rollback(&mut app),
                        _ => {}
                    }
                }
            }
        }

        update(&mut app);

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mode = if cli.rollback {
        AppMode::Rollback
    } else if cli.dry_run {
        AppMode::DryRun
    } else {
        AppMode::Deploy
    };

    let mut app = AppState::new(mode);

    if cli.rollback {
        start_rollback(&mut app);
    } else if cli.dry_run {
        app.current_step = AppStep::BackingUp;
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        prev_hook(panic);
    }));

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
