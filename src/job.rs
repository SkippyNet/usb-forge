//! The build pipeline, run on a background thread.
//!
//! Order matters for safety: everything is resolved, downloaded and verified *before* the
//! drive is touched, and the target disk is re-verified immediately before formatting.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::catalog::{self, Channel, Resolved, ToolId};
use crate::disks::{self, Disk};
use crate::net;
use crate::platform;
use crate::sys::human_size;
use crate::ventoy::{self, FileSystem};

#[derive(Clone)]
pub struct Plan {
    pub disk: Disk,
    pub tools: Vec<(ToolId, Channel)>,
    pub fs: FileSystem,
    pub gpt: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Active,
    Done,
    Failed,
}

pub struct Step {
    pub label: String,
    pub state: StepState,
}

#[derive(Default)]
pub struct Status {
    pub steps: Vec<Step>,
    pub detail: String,
    pub progress: Option<f32>,
    pub log: Vec<String>,
    /// `Some(Ok(drive))` on success, `Some(Err(msg))` on failure.
    pub finished: Option<Result<String, String>>,
    /// The disk has been formatted (cancelling is no longer harmless).
    pub formatting: bool,
}

pub struct Job {
    pub status: Arc<Mutex<Status>>,
    pub cancel: Arc<AtomicBool>,
}

impl Job {
    pub fn start(plan: Plan, repaint: impl Fn() + Send + 'static) -> Job {
        let mut steps = vec![
            "Look up latest versions".to_string(),
            "Download boot loader".into(),
        ];
        for (id, _) in &plan.tools {
            steps.push(format!("Download {}", catalog::info(*id).name));
        }
        steps.push(format!(
            "Format {} and install boot loader",
            plan.disk.display_id()
        ));
        steps.push("Copy and verify ISOs on the USB drive".into());
        let status = Arc::new(Mutex::new(Status {
            steps: steps
                .into_iter()
                .map(|label| Step {
                    label,
                    state: StepState::Pending,
                })
                .collect(),
            ..Default::default()
        }));
        let cancel = Arc::new(AtomicBool::new(false));
        let ctx = Ctx {
            status: status.clone(),
            cancel: cancel.clone(),
            repaint: Box::new(repaint),
        };
        std::thread::spawn(move || {
            let result = run(&plan, &ctx);
            if let Err(e) = &result {
                ctx.log(format!("ERROR: {e}"));
                ctx.with(|s| {
                    if let Some(step) = s.steps.iter_mut().find(|s| s.state == StepState::Active) {
                        step.state = StepState::Failed;
                    }
                });
            }
            ctx.with(|s| {
                s.progress = None;
                s.finished = Some(result);
            });
        });
        Job { status, cancel }
    }
}

struct Ctx {
    status: Arc<Mutex<Status>>,
    cancel: Arc<AtomicBool>,
    repaint: Box<dyn Fn() + Send>,
}

impl Ctx {
    fn with(&self, f: impl FnOnce(&mut Status)) {
        if let Ok(mut s) = self.status.lock() {
            f(&mut s);
        }
        (self.repaint)();
    }
    fn log(&self, line: impl Into<String>) {
        let line = line.into();
        self.with(|s| s.log.push(line));
    }
    fn begin(&self, step: usize, detail: impl Into<String>) {
        let detail = detail.into();
        self.with(|s| {
            s.steps[step].state = StepState::Active;
            s.detail = detail;
            s.progress = None;
        });
    }
    fn finish(&self, step: usize) {
        self.with(|s| s.steps[step].state = StepState::Done);
    }
    fn detail(&self, detail: impl Into<String>) {
        let detail = detail.into();
        self.with(|s| s.detail = detail);
    }
    fn progress_fn(&self, prefix: String) -> impl FnMut(u64, Option<u64>) + '_ {
        let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
        move |done, total| {
            if last.elapsed().as_millis() < 100 {
                return;
            }
            last = std::time::Instant::now();
            let text = match total {
                Some(t) => format!("{prefix}  {} / {}", human_size(done), human_size(t)),
                None => format!("{prefix}  {}", human_size(done)),
            };
            let frac = total.filter(|t| *t > 0).map(|t| done as f32 / t as f32);
            self.with(|s| {
                s.detail = text;
                s.progress = frac;
            });
        }
    }
    fn check_cancel(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::Relaxed) {
            Err("Cancelled.".into())
        } else {
            Ok(())
        }
    }
}

fn run(plan: &Plan, ctx: &Ctx) -> Result<String, String> {
    let cache = platform::cache_dir();
    std::fs::create_dir_all(&cache).map_err(|e| format!("Cannot create cache folder: {e}"))?;
    ctx.log(format!("Cache folder: {}", cache.display()));
    let n_tools = plan.tools.len();
    let step_format = 2 + n_tools;
    let step_copy = step_format + 1;

    // 1. Resolve every version up front.
    ctx.begin(0, "Asking each project for its latest release...");
    let ventoy_rel = ventoy::latest_release()?;
    ctx.log(format!("Boot loader {}", ventoy_rel.version));
    let mut resolved: Vec<(ToolId, Resolved)> = Vec::new();
    for (id, ch) in &plan.tools {
        ctx.check_cancel()?;
        let name = catalog::info(*id).name;
        ctx.detail(format!("Checking {name}..."));
        let r = catalog::resolve(*id, *ch).map_err(|e| format!("{name}: {e}"))?;
        ctx.log(format!(
            "{name} {} ({})",
            r.version,
            r.size
                .map(human_size)
                .unwrap_or_else(|| "size unknown".into())
        ));
        resolved.push((*id, r));
    }
    // Fail early, before any formatting, if something cannot fit.
    if let Some(max) = plan.fs.max_file() {
        for (id, r) in &resolved {
            if r.size.is_some_and(|s| s > max) {
                return Err(format!(
                    "{} ({}) is larger than 4 GB and cannot be stored on FAT32. Choose exFAT or NTFS, or deselect it.",
                    catalog::info(*id).name,
                    human_size(r.size.unwrap_or(0))
                ));
            }
        }
    }
    let total: u64 = resolved.iter().filter_map(|(_, r)| r.size).sum();
    let usable = plan.disk.size.saturating_sub(64 * 1024 * 1024);
    if total > usable {
        return Err(format!(
            "The selected software needs {} but the drive only holds about {}.",
            human_size(total),
            human_size(usable)
        ));
    }
    ctx.finish(0);

    // 2. Ventoy.
    ctx.begin(1, format!("Boot loader {}", ventoy_rel.version));
    let ventoy_dir = {
        let mut p = ctx.progress_fn(format!("Boot loader {}", ventoy_rel.version));
        ventoy::prepare(&ventoy_rel, &cache, &mut p, &ctx.cancel)?
    };
    ctx.log(format!(
        "Boot loader {} verified (SHA-256 OK)",
        ventoy_rel.version
    ));
    ctx.finish(1);

    // 3. Downloads (reusing verified cache entries).
    for (i, (id, r)) in resolved.iter().enumerate() {
        let step = 2 + i;
        let name = catalog::info(*id).name;
        ctx.begin(step, format!("{name} {}", r.version));
        fetch(ctx, &cache, name, r)?;
        ctx.finish(step);
    }

    // 4. Re-verify the disk, then format. Point of no return.
    ctx.check_cancel()?;
    ctx.begin(step_format, "Re-checking the target disk...");
    let disk = disks::reverify(&plan.disk)?;
    ctx.log(format!(
        "{} re-verified: {} / {} / bus {} / not boot, not system",
        disk.display_id(),
        disk.name,
        human_size(disk.size),
        disk.bus
    ));
    ctx.with(|s| s.formatting = true);
    ctx.detail(format!(
        "Erasing {} and installing the boot loader ({}, {})...",
        disk.display_id(),
        plan.fs.cli(),
        if plan.gpt { "GPT" } else { "MBR" }
    ));
    platform::install_ventoy(&ventoy_dir, &disk, plan.fs, plan.gpt, &mut |f| {
        ctx.with(|s| s.progress = Some(f));
    })?;
    ctx.log(format!(
        "Boot loader {} installed on {}",
        ventoy_rel.version,
        disk.display_id()
    ));
    ctx.detail("Waiting for Windows to mount the new partition...");
    let root = platform::data_partition(&disk)?;
    ctx.log(format!("Data partition mounted at {}", root.display()));
    ctx.finish(step_format);

    // 5. Copy + verify.
    ctx.begin(step_copy, "Copying...");
    for (id, r) in &resolved {
        let name = catalog::info(*id).name;
        copy_to_usb(ctx, &cache, &root, name, r)?;
    }
    ctx.finish(step_copy);
    ctx.log(format!("All done. {} is ready to boot.", root.display()));
    Ok(root.display().to_string())
}

fn fetch(ctx: &Ctx, cache: &Path, name: &str, r: &Resolved) -> Result<(), String> {
    let path = cache.join(&r.download_name);
    if path.exists() {
        let mut p = ctx.progress_fn(format!("Verifying cached {name}"));
        let hash = net::sha256_file(&path, &mut p, &ctx.cancel)?;
        if hash.eq_ignore_ascii_case(&r.sha256) {
            ctx.log(format!("{name}: using cached copy (SHA-256 OK)"));
            return Ok(());
        }
        ctx.log(format!("{name}: cached copy is stale, downloading again"));
    }
    let mut p = ctx.progress_fn(format!("Downloading {name} {}", r.version));
    net::download_verified(&r.url, &path, &r.sha256, &mut p, &ctx.cancel)?;
    ctx.log(format!("{name} {} downloaded (SHA-256 OK)", r.version));
    Ok(())
}

fn copy_to_usb(
    ctx: &Ctx,
    cache: &Path,
    root: &Path,
    name: &str,
    r: &Resolved,
) -> Result<(), String> {
    let src = cache.join(&r.download_name);
    let dest = root.join(&r.iso_name);
    let label = format!("Copying {name}");
    let expected = if r.zipped {
        // Memtest86+ ships its ISO inside a verified zip: hash the ISO as it is extracted,
        // then confirm the copy on the drive matches.
        let file = std::fs::File::open(&src).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("{name}: {e}"))?;
        let idx = (0..zip.len())
            .find(|&i| {
                zip.name_for_index(i)
                    .is_some_and(|n| n.is_ok_and(|n| n.to_ascii_lowercase().ends_with(".iso")))
            })
            .ok_or_else(|| format!("{name}: no ISO inside the zip"))?;
        let entry = zip.by_index(idx).map_err(|e| e.to_string())?;
        let size = entry.size();
        let mut p = ctx.progress_fn(label);
        net::copy_hashed(entry, Some(size), &dest, &mut p, &ctx.cancel)?
    } else {
        let file = std::fs::File::open(&src).map_err(|e| e.to_string())?;
        let size = file.metadata().ok().map(|m| m.len());
        let mut p = ctx.progress_fn(label);
        let written = net::copy_hashed(file, size, &dest, &mut p, &ctx.cancel)?;
        if !written.eq_ignore_ascii_case(&r.sha256) {
            return Err(format!("{name}: source changed while copying."));
        }
        r.sha256.clone()
    };
    let mut p = ctx.progress_fn(format!("Verifying {name} on USB"));
    let on_usb = net::sha256_file(&dest, &mut p, &ctx.cancel)?;
    if !on_usb.eq_ignore_ascii_case(&expected) {
        return Err(format!(
            "{name}: the copy on the USB drive does not match (bad drive?)."
        ));
    }
    ctx.log(format!("{} -> {} (verified)", name, dest.display()));
    Ok(())
}
