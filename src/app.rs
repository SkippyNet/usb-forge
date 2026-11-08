//! The user interface.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{Align, Color32, Layout, Margin, RichText, Stroke};

use crate::catalog::{self, Category, Channel, Resolved, TOOLS, ToolId};
use crate::disks::{self, Disk};
use crate::job::{Job, Plan, StepState};
use crate::platform;
use crate::sys::human_size;
use crate::theme::{self, ACCENT, BORDER, CARD, CARD_HOVER, DANGER, MUTED, OK, PANEL, WARN};
use crate::updater::{self, Update};
use crate::ventoy::FileSystem;

enum UpdateState {
    Checking(Receiver<Result<Option<Update>, String>>),
    Idle,
    Available(Update),
    Installing(
        Receiver<Result<std::path::PathBuf, String>>,
        Arc<Mutex<String>>,
    ),
}

const CONFIRM_WORD: &str = "ERASE";
const SECURE_BOOT_GUIDE: &str =
    "https://github.com/SkippyNet/usb-forge/blob/main/docs/SECURE_BOOT.md";

enum Lookup {
    Loading,
    Ready(Resolved),
    Failed(String),
}

type Lookups = Arc<Mutex<HashMap<(ToolId, Channel), Lookup>>>;

struct DiskScan {
    targets: Vec<Disk>,
    hidden: usize,
}

pub struct UsbForgeApp {
    needs_restart: bool,
    secure_boot: Option<bool>,
    update: UpdateState,
    scan: Option<Result<DiskScan, String>>,
    scanning: Option<Receiver<Result<DiskScan, String>>>,
    last_scan: Instant,
    selected_disk: Option<String>,
    chosen: HashMap<ToolId, bool>,
    channels: HashMap<ToolId, Channel>,
    fs: FileSystem,
    gpt: bool,
    confirm: String,
    lookups: Lookups,
    job: Option<Job>,
    notice: Option<String>,
}

impl UsbForgeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let update = {
            let (tx, rx) = mpsc::channel();
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(updater::check());
                ctx.request_repaint();
            });
            UpdateState::Checking(rx)
        };
        let mut app = UsbForgeApp {
            needs_restart: platform::needs_admin_restart(),
            secure_boot: platform::secure_boot_enabled(),
            update,
            scan: None,
            scanning: None,
            last_scan: Instant::now(),
            selected_disk: None,
            chosen: TOOLS.iter().map(|t| (t.id, t.default_on)).collect(),
            channels: TOOLS
                .iter()
                .filter(|t| t.has_channel)
                .map(|t| (t.id, Channel::Lts))
                .collect(),
            fs: FileSystem::ExFat,
            gpt: false,
            confirm: String::new(),
            lookups: Arc::new(Mutex::new(HashMap::new())),
            job: None,
            notice: None,
        };
        app.start_scan(&cc.egui_ctx);
        app.check_versions(&cc.egui_ctx);
        app
    }

    fn channel(&self, id: ToolId) -> Channel {
        self.channels.get(&id).copied().unwrap_or(Channel::Latest)
    }

    // ---- background work ---------------------------------------------------------------

    fn start_scan(&mut self, ctx: &egui::Context) {
        if self.scanning.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = disks::all_disks().map(|all| {
                let total = all.len();
                let targets: Vec<Disk> = all.into_iter().filter(Disk::is_safe_target).collect();
                DiskScan {
                    hidden: total - targets.len(),
                    targets,
                }
            });
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.scanning = Some(rx);
        self.last_scan = Instant::now();
    }

    fn poll_scan(&mut self) {
        let Some(rx) = &self.scanning else { return };
        if let Ok(result) = rx.try_recv() {
            self.scanning = None;
            if let Ok(scan) = &result {
                if let Some(sel) = &self.selected_disk
                    && !scan.targets.iter().any(|d| &d.id == sel)
                {
                    self.selected_disk = None;
                }
                if self.selected_disk.is_none() && scan.targets.len() == 1 {
                    self.selected_disk = Some(scan.targets[0].id.clone());
                }
            }
            self.scan = Some(result);
        }
    }

    fn check_versions(&mut self, ctx: &egui::Context) {
        let mut wanted = Vec::new();
        for t in TOOLS {
            if t.has_channel {
                wanted.push((t.id, Channel::Lts));
                wanted.push((t.id, Channel::Latest));
            } else {
                wanted.push((t.id, Channel::Latest));
            }
        }
        for key in wanted {
            if let Ok(mut map) = self.lookups.lock() {
                if matches!(map.get(&key), Some(Lookup::Loading)) {
                    continue;
                }
                map.insert(key, Lookup::Loading);
            }
            let lookups = self.lookups.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let result = match catalog::resolve(key.0, key.1) {
                    Ok(r) => Lookup::Ready(r),
                    Err(e) => Lookup::Failed(e),
                };
                if let Ok(mut map) = lookups.lock() {
                    map.insert(key, result);
                }
                ctx.request_repaint();
            });
        }
    }

    fn selected_tools(&self) -> Vec<(ToolId, Channel)> {
        TOOLS
            .iter()
            .filter(|t| self.chosen.get(&t.id).copied().unwrap_or(false))
            .map(|t| (t.id, self.channel(t.id)))
            .collect()
    }

    fn selected_disk(&self) -> Option<&Disk> {
        let scan = self.scan.as_ref()?.as_ref().ok()?;
        scan.targets
            .iter()
            .find(|d| Some(&d.id) == self.selected_disk.as_ref())
    }

    fn poll_update(&mut self, ctx: &egui::Context) {
        match &self.update {
            UpdateState::Checking(rx) => {
                if let Ok(result) = rx.try_recv() {
                    self.update = match result {
                        Ok(Some(u)) => UpdateState::Available(u),
                        // A failed check (offline, rate-limited) is not worth bothering the user.
                        Ok(None) | Err(_) => UpdateState::Idle,
                    };
                }
            }
            UpdateState::Installing(rx, _) => {
                if let Ok(result) = rx.try_recv() {
                    match result.and_then(|exe| updater::restart(&exe)) {
                        Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                        Err(e) => {
                            self.notice = Some(e);
                            self.update = UpdateState::Idle;
                        }
                    }
                }
            }
            UpdateState::Idle | UpdateState::Available(_) => {}
        }
    }

    fn start_update(&mut self, ctx: &egui::Context, update: Update) {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new("Downloading update...".to_string()));
        let shared = status.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let mut progress = |done: u64, total: Option<u64>| {
                if let Ok(mut s) = shared.lock() {
                    *s = match total {
                        Some(t) => format!(
                            "Downloading v{}  {} / {}",
                            update.version,
                            human_size(done),
                            human_size(t)
                        ),
                        None => format!("Downloading v{}  {}", update.version, human_size(done)),
                    };
                }
                ctx.request_repaint();
            };
            let _ = tx.send(updater::install(&update, &mut progress, &cancel));
            ctx.request_repaint();
        });
        self.update = UpdateState::Installing(rx, status);
    }

    /// Known size of the selection, how many sizes are still unknown, and blocking problems.
    fn assess(&self) -> (u64, usize, Vec<String>) {
        let mut total = 0;
        let mut unknown = 0;
        let mut problems = Vec::new();
        let map = self.lookups.lock().ok();
        for (id, ch) in self.selected_tools() {
            match map.as_ref().and_then(|m| m.get(&(id, ch))) {
                Some(Lookup::Ready(r)) => match r.size {
                    Some(s) => {
                        total += s;
                        if self.fs.max_file().is_some_and(|max| s > max) {
                            problems.push(format!(
                                "{} is {} — too big for FAT32. Pick exFAT/NTFS or deselect it.",
                                catalog::info(id).name,
                                human_size(s)
                            ));
                        }
                    }
                    None => unknown += 1,
                },
                _ => unknown += 1,
            }
        }
        if let Some(disk) = self.selected_disk()
            && total > disk.size.saturating_sub(64 * 1024 * 1024)
        {
            problems.push(format!(
                "The selection needs {} but the drive holds {}.",
                human_size(total),
                human_size(disk.size)
            ));
        }
        (total, unknown, problems)
    }

    // ---- views -------------------------------------------------------------------------

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("⚡").size(26.0).color(ACCENT));
            ui.vertical(|ui| {
                ui.label(RichText::new("USB Forge").size(22.0).strong());
                ui.label(
                    RichText::new("Build a verified multi-boot USB drive with Ventoy")
                        .color(MUTED)
                        .small(),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.needs_restart {
                    if ui.button("Restart as administrator").clicked() {
                        match platform::relaunch_elevated() {
                            Ok(()) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                            Err(e) => self.notice = Some(e),
                        }
                    }
                    chip(ui, "⚠ Not elevated", WARN);
                } else {
                    chip(ui, &format!("✔ {}", platform::privilege_note()), OK);
                }
                if self.secure_boot == Some(true) {
                    ui.add_space(4.0);
                    if ui
                        .small_button("How to turn it off")
                        .on_hover_text(
                            "Opens a guide for common PC vendors. Firmware menus vary by model, \
                             so the steps may not exactly match your PC.",
                        )
                        .clicked()
                    {
                        platform::open_url(SECURE_BOOT_GUIDE);
                    }
                    chip(ui, "⚠ Secure Boot is on", WARN).on_hover_text(
                        "Secure Boot is turned on in this PC's firmware. Some tools on a USB Forge \
                         drive only start with Secure Boot turned off. This applies to the PC you \
                         boot the USB drive on.",
                    );
                }
                ui.add_space(8.0);
                let mut start = None;
                match &self.update {
                    UpdateState::Available(u) => {
                        let button = egui::Button::new(
                            RichText::new(format!("Update to v{} & restart", u.version))
                                .color(Color32::BLACK)
                                .strong(),
                        )
                        .fill(OK);
                        let resp = ui
                            .add_enabled(self.job.is_none(), button)
                            .on_hover_text(format!(
                                "Signed release, verified before installing.\n{}",
                                u.page_url
                            ))
                            .on_disabled_hover_text("Finish the current build first.");
                        if resp.clicked() {
                            start = Some(u.clone());
                        }
                    }
                    UpdateState::Installing(_, status) => {
                        ui.add(egui::Spinner::new());
                        if let Ok(s) = status.lock() {
                            ui.label(RichText::new(s.as_str()).color(MUTED).small());
                        }
                    }
                    UpdateState::Checking(_) | UpdateState::Idle => {
                        ui.label(
                            RichText::new(format!("v{}", updater::CURRENT_VERSION))
                                .color(MUTED)
                                .small(),
                        );
                    }
                }
                if let Some(u) = start {
                    self.start_update(ui.ctx(), u);
                }
            });
        });
    }

    fn drives_panel(&mut self, ui: &mut egui::Ui) {
        theme::section(ui, "1", "Choose a USB drive");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.scanning.is_none(), egui::Button::new("⟳ Rescan"))
                .clicked()
            {
                self.start_scan(ui.ctx());
            }
            if self.scanning.is_some() {
                ui.add(egui::Spinner::new());
            }
        });
        ui.add_space(4.0);

        let mut clicked = None;
        match &self.scan {
            None => {
                ui.label(RichText::new("Looking for drives...").color(MUTED));
            }
            Some(Err(e)) => {
                ui.label(RichText::new(e).color(DANGER));
            }
            Some(Ok(scan)) => {
                if scan.targets.is_empty() {
                    theme::card(CARD, BORDER).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("No USB drives found").strong());
                        ui.label(
                            RichText::new(
                                "Plug in a thumb drive. The list refreshes automatically.",
                            )
                            .color(MUTED)
                            .small(),
                        );
                    });
                }
                for d in &scan.targets {
                    let selected = Some(&d.id) == self.selected_disk.as_ref();
                    if drive_card(ui, d, selected) {
                        clicked = Some(d.id.clone());
                    }
                }
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!(
                        "🔒 {} internal or system disk(s) hidden for safety",
                        scan.hidden
                    ))
                    .color(MUTED)
                    .small(),
                );
            }
        }
        if let Some(n) = clicked {
            self.selected_disk = Some(n);
        }

        ui.add_space(14.0);
        theme::section(ui, "2", "Format options");
        ui.label(RichText::new("File system").color(MUTED).small());
        if !platform::SUPPORTED_FS.contains(&self.fs) {
            self.fs = platform::SUPPORTED_FS[0];
        }
        egui::ComboBox::from_id_salt("fs")
            .width(ui.available_width())
            .selected_text(self.fs.cli())
            .show_ui(ui, |ui| {
                for fs in platform::SUPPORTED_FS {
                    ui.selectable_value(&mut self.fs, *fs, fs.describe());
                }
            });
        ui.label(RichText::new(self.fs.describe()).color(MUTED).small());
        if platform::SUPPORTED_FS.len() == 1 {
            ui.label(
                RichText::new(format!(
                    "Ventoy's {} installer always uses exFAT.",
                    platform::OS_NAME
                ))
                .color(MUTED)
                .small(),
            );
        }
        ui.add_space(4.0);
        ui.label(RichText::new("Partition table").color(MUTED).small());
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.gpt, false, "MBR (most compatible)");
            ui.selectable_value(&mut self.gpt, true, "GPT");
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Ventoy boots on both UEFI and legacy BIOS PCs, with Secure Boot support. \
                 Add more ISOs later by copying them onto the drive.",
            )
            .color(MUTED)
            .small(),
        );
    }

    fn tools_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            theme::section(ui, "3", "Choose software");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("⟳ Check versions").clicked() {
                    self.check_versions(ui.ctx());
                }
                if ui.button("None").clicked() {
                    self.chosen.values_mut().for_each(|v| *v = false);
                }
            });
        });
        ui.label(
            RichText::new(
                "Each item is downloaded from its official project and checked against the \
                 project's published SHA-256 checksum.",
            )
            .color(MUTED)
            .small(),
        );
        ui.add_space(6.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for cat in Category::ALL {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(cat.label().to_uppercase())
                            .color(MUTED)
                            .small()
                            .strong(),
                    );
                    for tool in TOOLS.iter().filter(|t| t.category == cat) {
                        self.tool_row(ui, tool);
                    }
                }
                ui.add_space(8.0);
            });
    }

    fn tool_row(&mut self, ui: &mut egui::Ui, tool: &catalog::ToolInfo) {
        let mut on = self.chosen.get(&tool.id).copied().unwrap_or(false);
        let mut channel = self.channel(tool.id);
        let lookup_text = {
            let map = self.lookups.lock().ok();
            match map.as_ref().and_then(|m| m.get(&(tool.id, channel))) {
                Some(Lookup::Ready(r)) => Some(Ok(format!(
                    "{}  ·  {}",
                    r.version,
                    r.size.map(human_size).unwrap_or_else(|| "size ?".into())
                ))),
                Some(Lookup::Failed(e)) => Some(Err(e.clone())),
                Some(Lookup::Loading) | None => None,
            }
        };
        let too_big = matches!(&lookup_text, Some(Ok(_)))
            && self.fs.max_file().is_some_and(|max| {
                self.lookups.lock().ok().is_some_and(|m| {
                    matches!(m.get(&(tool.id, channel)), Some(Lookup::Ready(r)) if r.size.is_some_and(|s| s > max))
                })
            });

        let (fill, stroke) = if on {
            (CARD_HOVER, ACCENT)
        } else {
            (CARD, BORDER)
        };
        theme::card(fill, stroke).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.checkbox(&mut on, RichText::new(tool.name).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if tool.has_channel {
                        egui::ComboBox::from_id_salt(("ch", tool.name))
                            .width(86.0)
                            .selected_text(channel.label())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut channel, Channel::Lts, "LTS");
                                ui.selectable_value(&mut channel, Channel::Latest, "Latest");
                            });
                    }
                    match &lookup_text {
                        None => {
                            ui.add(egui::Spinner::new().size(14.0));
                        }
                        Some(Ok(t)) => {
                            let color = if too_big { DANGER } else { MUTED };
                            ui.label(RichText::new(t).color(color).small());
                        }
                        Some(Err(e)) => {
                            ui.label(RichText::new("lookup failed").color(DANGER).small())
                                .on_hover_text(e);
                        }
                    }
                });
            });
            ui.label(RichText::new(tool.blurb).color(MUTED).small());
        });
        self.chosen.insert(tool.id, on);
        if tool.has_channel {
            self.channels.insert(tool.id, channel);
        }
    }

    fn footer(&mut self, ui: &mut egui::Ui) {
        let tools = self.selected_tools();
        let (total, unknown, problems) = self.assess();
        let disk = self.selected_disk().cloned();

        if let Some(n) = &self.notice {
            ui.label(RichText::new(n).color(DANGER));
        }
        for p in &problems {
            ui.label(RichText::new(format!("⚠ {p}")).color(WARN));
        }

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                let summary = match &disk {
                    Some(d) => format!("{} · {} · {}", d.display_id(), d.name, human_size(d.size)),
                    None => "No drive selected".into(),
                };
                ui.label(RichText::new(summary).strong());
                let mut size_line = format!("{} item(s) · {}", tools.len(), human_size(total));
                if unknown > 0 {
                    size_line.push_str(&format!(" + {unknown} pending"));
                }
                if let Some(d) = &disk {
                    size_line.push_str(&format!(" of {}", human_size(d.size)));
                }
                ui.label(RichText::new(size_line).color(MUTED).small());
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ready = !self.needs_restart
                    && disk.is_some()
                    && !tools.is_empty()
                    && problems.is_empty()
                    && self.confirm.trim() == CONFIRM_WORD;
                let button = egui::Button::new(
                    RichText::new("  Build USB drive  ")
                        .size(16.0)
                        .strong()
                        .color(if ready { Color32::BLACK } else { MUTED }),
                )
                .fill(if ready { ACCENT } else { theme::ACCENT_DIM })
                .min_size(egui::vec2(190.0, 40.0));
                let resp = ui.add_enabled(ready, button);
                let hint = if self.needs_restart {
                    "Restart as administrator first (top right)."
                } else if disk.is_none() {
                    "Select a USB drive."
                } else if tools.is_empty() {
                    "Select at least one item."
                } else if !problems.is_empty() {
                    "Resolve the warnings above."
                } else {
                    "Type ERASE to confirm."
                };
                let resp = resp.on_disabled_hover_text(hint);
                if resp.clicked()
                    && let Some(d) = disk.clone()
                {
                    let ctx = ui.ctx().clone();
                    self.notice = None;
                    self.job = Some(Job::start(
                        Plan {
                            disk: d,
                            tools: tools.clone(),
                            fs: self.fs,
                            gpt: self.gpt,
                        },
                        move || ctx.request_repaint(),
                    ));
                }

                ui.add(
                    egui::TextEdit::singleline(&mut self.confirm)
                        .hint_text(CONFIRM_WORD)
                        .desired_width(90.0),
                );
                let warn = match &disk {
                    Some(d) => format!(
                        "Everything on {} ({}) will be erased. Type",
                        d.display_id(),
                        d.mounts_label()
                    ),
                    None => "Type".into(),
                };
                ui.label(RichText::new(warn).color(WARN).small());
            });
        });
    }

    fn progress_view(&mut self, ui: &mut egui::Ui) {
        let Some(job) = &self.job else { return };
        let mut back = false;
        let mut cancel = false;
        if let Ok(st) = job.status.lock() {
            ui.label(RichText::new("Building your USB drive").size(20.0).strong());
            ui.add_space(8.0);
            ui.columns(2, |cols| {
                theme::card(PANEL, BORDER).show(&mut cols[0], |ui| {
                    ui.set_width(ui.available_width());
                    for step in &st.steps {
                        ui.horizontal(|ui| {
                            step_icon(ui, step.state);
                            let color = match step.state {
                                StepState::Pending => MUTED,
                                StepState::Failed => DANGER,
                                _ => theme::TEXT,
                            };
                            ui.label(RichText::new(&step.label).color(color));
                        });
                    }
                });
                let ui = &mut cols[1];
                theme::card(PANEL, BORDER).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(&st.detail).strong());
                    let bar = match st.progress {
                        Some(p) => egui::ProgressBar::new(p).show_percentage(),
                        None if st.finished.is_none() => egui::ProgressBar::new(0.0).animate(true),
                        None => egui::ProgressBar::new(1.0),
                    };
                    ui.add(bar.fill(ACCENT).desired_height(10.0));
                    match &st.finished {
                        Some(Ok(drive)) => {
                            ui.add_space(8.0);
                            ui.label(RichText::new("✔ Your USB drive is ready").size(18.0).color(OK).strong());
                            ui.label(format!(
                                "Boot from it and choose an ISO from the Ventoy menu. Drop more ISOs onto {drive} any time."
                            ));
                        }
                        Some(Err(e)) => {
                            ui.add_space(8.0);
                            ui.label(RichText::new("✖ Build failed").size(18.0).color(DANGER).strong());
                            ui.label(RichText::new(e).color(DANGER));
                            if !st.formatting {
                                ui.label(RichText::new("The drive was not modified.").color(MUTED));
                            }
                        }
                        None => {}
                    }
                });
            });

            ui.add_space(8.0);
            ui.label(RichText::new("Activity").color(MUTED).small().strong());
            theme::card(egui::Color32::from_rgb(0x0b, 0x0d, 0x12), BORDER).show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .max_height((ui.available_height() - 60.0).max(120.0))
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for line in &st.log {
                            let color = if line.starts_with("ERROR") {
                                DANGER
                            } else {
                                MUTED
                            };
                            ui.label(RichText::new(line).monospace().color(color));
                        }
                    });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if st.finished.is_some() {
                    if ui.button("  Done  ").clicked() {
                        back = true;
                    }
                } else if !st.formatting {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                } else {
                    ui.label(
                        RichText::new("Do not unplug the drive while it is being written.")
                            .color(WARN),
                    );
                }
            });
        }
        if cancel {
            job.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if back {
            self.job = None;
            self.confirm.clear();
            self.start_scan(ui.ctx());
        }
    }
}

impl eframe::App for UsbForgeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_scan();
        self.poll_update(ui.ctx());
        // Hot-plug: rescan every few seconds while idle.
        if self.job.is_none() && self.last_scan.elapsed() > Duration::from_secs(4) {
            self.start_scan(ui.ctx());
        }
        ui.ctx().request_repaint_after(Duration::from_secs(1));

        egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(20, 12)),
            )
            .show(ui, |ui| self.header(ui));

        if self.job.is_some() {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(theme::BG)
                        .inner_margin(Margin::same(20)),
                )
                .show(ui, |ui| self.progress_view(ui));
            return;
        }

        egui::Panel::bottom("footer")
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(20, 12))
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| self.footer(ui));

        egui::Panel::left("drives")
            .resizable(false)
            .exact_size(340.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(Margin::same(20)),
            )
            .show(ui, |ui| self.drives_panel(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(Margin::same(20)),
            )
            .show(ui, |ui| self.tools_panel(ui));
    }
}

fn chip(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.15))
        .stroke(Stroke::new(1.0, color))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(color).small().strong());
        })
        .response
}

/// A selectable drive card. Returns true when clicked.
fn drive_card(ui: &mut egui::Ui, d: &Disk, selected: bool) -> bool {
    let (fill, stroke) = if selected {
        (CARD_HOVER, ACCENT)
    } else {
        (CARD, BORDER)
    };
    let inner = theme::card(fill, stroke).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("🖴")
                    .size(22.0)
                    .color(if selected { ACCENT } else { MUTED }),
            );
            ui.vertical(|ui| {
                ui.label(RichText::new(&d.name).strong());
                ui.label(
                    RichText::new(format!(
                        "{} · {} · {} · {}",
                        d.display_id(),
                        human_size(d.size),
                        d.mounts_label(),
                        d.style
                    ))
                    .color(MUTED)
                    .small(),
                );
            });
        });
        if !d.looks_like_thumb_drive() {
            ui.label(
                RichText::new("⚠ Large USB disk. Make sure this isn't an external backup drive.")
                    .color(WARN)
                    .small(),
            );
        }
    });
    inner
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

fn step_icon(ui: &mut egui::Ui, state: StepState) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
    let c = rect.center();
    let p = ui.painter();
    match state {
        StepState::Pending => {
            p.circle_stroke(c, 6.0, Stroke::new(1.5, MUTED));
        }
        StepState::Active => {
            p.circle_filled(c, 6.0, ACCENT);
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        StepState::Done => {
            p.circle_filled(c, 7.0, OK);
            p.line_segment(
                [c + egui::vec2(-3.5, 0.0), c + egui::vec2(-1.0, 2.5)],
                Stroke::new(2.0, theme::BG),
            );
            p.line_segment(
                [c + egui::vec2(-1.0, 2.5), c + egui::vec2(3.5, -2.5)],
                Stroke::new(2.0, theme::BG),
            );
        }
        StepState::Failed => {
            p.circle_filled(c, 7.0, DANGER);
            p.line_segment(
                [c + egui::vec2(-3.0, -3.0), c + egui::vec2(3.0, 3.0)],
                Stroke::new(2.0, theme::BG),
            );
            p.line_segment(
                [c + egui::vec2(-3.0, 3.0), c + egui::vec2(3.0, -3.0)],
                Stroke::new(2.0, theme::BG),
            );
        }
    }
}
