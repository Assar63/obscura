use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use obsbot_core::transport::uvc_xu;
use obsbot_core::v4l2::UvcQuery;
use obsbot_core::{discover, CameraInfo, Device, FeatureId, FeatureKind};

#[derive(Parser)]
#[command(version, about = "Control OBSBOT webcams on Linux")]
struct Cli {
    /// Video node to use (default: first OBSBOT camera found).
    #[arg(short, long, global = true)]
    device: Option<PathBuf>,
    /// Include non-OBSBOT cameras (standard UVC controls only).
    #[arg(short, long, global = true)]
    all: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List connected cameras.
    List,
    /// Show device details and the matched profile.
    Info,
    /// Print every feature and its current value.
    Dump {
        #[arg(long)]
        json: bool,
        /// Also list features the device doesn't support.
        #[arg(long)]
        unsupported: bool,
    },
    /// Read one feature.
    Get { feature: String },
    /// Write one feature: `on`/`off`, a choice label, or a number in display
    /// units (e.g. `zoom 1.5`).
    Set {
        feature: String,
        value: String,
        /// Wait for the camera's status to catch up, then report whether it
        /// took the new value.
        #[arg(long)]
        check: bool,
    },
    /// Check OBSBOT's download page for newer firmware (needs network
    /// access). Only checks; update with OBSBOT Center.
    Firmware,
    /// Show the camera's live state (power, AI mode, zoom, stream fps…)
    /// decoded from its status block.
    Status {
        /// Also print the raw status block.
        #[arg(long)]
        raw: bool,
    },
    /// List the raw V4L2 controls the driver exposes.
    Controls,
    /// Gimbal presets stored on the camera (slots are numbered from 1).
    #[command(subcommand)]
    Preset(Preset),
    /// Move the gimbal at a velocity for a while, then stop. RIGHT and UP
    /// are -1..1 fractions of the maximum speed.
    Gimbal {
        #[arg(allow_hyphen_values = true)]
        right: f32,
        #[arg(allow_hyphen_values = true)]
        up: f32,
        /// How long to move, in milliseconds.
        #[arg(long, default_value_t = 500)]
        ms: u64,
    },
    /// Low-level UVC Extension Unit access (development only).
    #[command(subcommand, hide = true)]
    Raw(Raw),
}

// Variant names become the subcommands `xu-probe`, `xu-get` and `xu-set`.
#[allow(clippy::enum_variant_names)]
#[derive(Subcommand)]
enum Preset {
    /// List the preset slots and their names.
    List,
    /// Store the current gimbal position and zoom in SLOT.
    Save {
        slot: u32,
        /// Defaults to "PresetN", like OBSBOT Center.
        name: Option<String>,
    },
    /// Move to the position stored in SLOT.
    Recall { slot: u32 },
    /// Rename SLOT.
    Rename { slot: u32, name: String },
}

/// Converts a 1-based slot number from the command line to the camera's.
fn slot_index(slot: u32) -> Result<u32> {
    slot.checked_sub(1)
        .ok_or_else(|| anyhow!("preset slots are numbered from 1"))
}

#[derive(Subcommand)]
enum Raw {
    /// Read-only scan for XU controls (GET_LEN/GET_INFO).
    XuProbe,
    /// GET_CUR/MIN/MAX/DEF/RES of an XU control, printed as hex.
    XuGet {
        #[arg(long)]
        unit: u8,
        #[arg(long)]
        selector: u8,
        #[arg(long, default_value = "cur", value_parser = ["cur", "min", "max", "def", "res"])]
        query: String,
    },
    /// SET_CUR an XU control with hex bytes. Only replay captured commands.
    XuSet {
        #[arg(long)]
        unit: u8,
        #[arg(long)]
        selector: u8,
        /// Hex payload, e.g. `aa55010002`; spaces allowed.
        data: String,
    },
    /// Send a framed query (selector 2) and print the response payload.
    /// Only use queries seen in captures: unknown ones have hung the camera.
    Query {
        /// Destination module, e.g. `04` (AI/gimbal), `02` (ISP), `0d` (system).
        dst: String,
        /// Command ID as captured, e.g. `0401`.
        cmd: String,
    },
}

fn pick_camera(cli: &Cli) -> Result<CameraInfo> {
    let cameras = discover(true);
    match &cli.device {
        Some(path) => cameras
            .into_iter()
            .find(|c| &c.path == path)
            .ok_or_else(|| anyhow!("{} is not a USB camera capture node", path.display())),
        None => cameras
            .into_iter()
            .find(|c| cli.all || c.is_obsbot)
            .ok_or_else(|| {
                anyhow!("no OBSBOT camera found (use --all to include other cameras, or `obsbotctl list --all`)")
            }),
    }
}

fn open(cli: &Cli) -> Result<Device> {
    let info = pick_camera(cli)?;
    let path = info.path.clone();
    Device::open_auto(info).with_context(|| format!("opening {}", path.display()))
}

fn feature_id(name: &str) -> Result<FeatureId> {
    FeatureId::from_name(name).ok_or_else(|| {
        let names: Vec<&str> = FeatureId::ALL.iter().map(|f| f.name()).collect();
        anyhow!(
            "unknown feature `{name}`; known features:\n  {}",
            names.join("\n  ")
        )
    })
}

fn describe_kind(kind: &FeatureKind) -> String {
    match kind {
        FeatureKind::Toggle => "on|off".into(),
        FeatureKind::Choice { options } => options
            .iter()
            .map(|o| o.label.as_str())
            .collect::<Vec<_>>()
            .join("|"),
        FeatureKind::Range { min, max, .. } => {
            format!("{}..{}", kind.format(*min), kind.format(*max))
        }
        FeatureKind::Action => "action".into(),
    }
}

fn parse_hex(s: &str) -> Result<Vec<u8>> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if !s.len().is_multiple_of(2) {
        bail!("hex payload must have an even number of digits");
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).context("invalid hex"))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Command::List => {
            let cameras = discover(cli.all);
            if cameras.is_empty() {
                println!(
                    "No cameras found{}.",
                    if cli.all { "" } else { " (try --all)" }
                );
            }
            for c in cameras {
                println!(
                    "{}  {:04x}:{:04x}  {}{}",
                    c.path.display(),
                    c.vendor_id,
                    c.product_id,
                    c.product.as_deref().unwrap_or(&c.name),
                    if c.is_obsbot { "" } else { "  (not OBSBOT)" }
                );
            }
        }
        Command::Info => {
            let dev = open(&cli)?;
            let i = &dev.info;
            println!("Device:        {}", i.path.display());
            println!("Name:          {}", i.name);
            println!(
                "USB ID:        {:04x}:{:04x} (bus path {})",
                i.vendor_id, i.product_id, i.usb_path
            );
            println!(
                "Manufacturer:  {}",
                i.manufacturer.as_deref().unwrap_or("-")
            );
            println!("Product:       {}", i.product.as_deref().unwrap_or("-"));
            let fw = dev.firmware_info();
            let serial = i
                .serial
                .clone()
                .or_else(|| fw.as_ref().and_then(|f| f.serial.clone()));
            println!("Serial:        {}", serial.as_deref().unwrap_or("-"));
            println!(
                "Firmware:      {}",
                fw.as_ref().map_or("-", |f| f.version.as_str())
            );
            println!("USB bcdDevice: {}", i.usb_version.as_deref().unwrap_or("-"));
            println!("Profile:       {} ({})", dev.profile.name, dev.profile.id);
        }
        Command::Dump { json, unsupported } => {
            let dev = open(&cli)?;
            let states: Vec<_> = dev
                .read_all()
                .into_iter()
                .filter(|s| s.supported || *unsupported)
                .collect();
            if *json {
                println!("{}", serde_json::to_string_pretty(&states)?);
            } else {
                for s in states {
                    let value = match (s.supported, s.value) {
                        (true, Some(v)) => s.kind.format(v),
                        _ => format!("- ({})", s.reason.as_deref().unwrap_or("unavailable")),
                    };
                    let inactive = if s.supported && !s.active {
                        "  [inactive]"
                    } else {
                        ""
                    };
                    println!(
                        "{:<28} {:<14} {}{}",
                        s.id.name(),
                        value,
                        describe_kind(&s.kind),
                        inactive
                    );
                }
            }
        }
        Command::Get { feature } => {
            let dev = open(&cli)?;
            let s = dev.feature(feature_id(feature)?);
            match (s.supported, s.value) {
                (true, Some(v)) => println!("{}", s.kind.format(v)),
                _ => bail!(
                    "{}: {}",
                    feature,
                    s.reason.unwrap_or_else(|| "unavailable".into())
                ),
            }
        }
        Command::Set {
            feature,
            value,
            check,
        } => {
            let dev = open(&cli)?;
            let id = feature_id(feature)?;
            let kind = dev.feature(id).kind;
            let raw = kind.parse(value).ok_or_else(|| {
                anyhow!(
                    "invalid value `{value}` for {feature} (expected {})",
                    describe_kind(&kind)
                )
            })?;
            let before = obsbot_core::log::since(0).last().map_or(0, |e| e.seq);
            let s = dev.set(id, raw)?;
            println!(
                "{} = {}",
                feature,
                s.value.map_or("?".into(), |v| s.kind.format(v))
            );
            if *check {
                std::thread::sleep(std::time::Duration::from_secs(3));
                dev.feature(id);
                let warnings: Vec<_> = obsbot_core::log::since(before)
                    .into_iter()
                    .filter(|e| e.level == obsbot_core::log::Level::Warn)
                    .collect();
                if warnings.is_empty() {
                    println!("checked: the camera took it");
                }
                for w in warnings {
                    eprintln!("warning: {}", w.message);
                }
            }
        }
        Command::Firmware => {
            let dev = open(&cli)?;
            let c = dev.check_firmware_update().map_err(|e| anyhow!(e))?;
            println!("Camera firmware:  {}", c.current);
            println!("Latest firmware:  {}", c.latest);
            if c.update_available {
                println!("An update is available. Install it with OBSBOT Center (Windows or");
                println!("macOS): {}", c.page);
            } else {
                println!("The camera is up to date.");
            }
        }
        Command::Status { raw } => {
            let dev = open(&cli)?;
            let Some(st) = dev.live_status() else {
                bail!("{} has no vendor status block", dev.profile.name);
            };
            println!("Power:        {}", st.power);
            println!(
                "AI mode:      {} (sub-mode {})",
                st.ai_mode_label, st.ai_sub_mode
            );
            println!("Zoom:         {:.2}x", st.zoom);
            println!("FOV:          {}", st.fov.unwrap_or("-"));
            println!("Stream fps:   {}", st.fps);
            println!("Light level:  {}", st.light_level);
            println!("Event count:  {}", st.event_count);
            if *raw {
                println!("Raw:          {}", st.raw);
            }
        }
        Command::Preset(p) => {
            let dev = open(&cli)?;
            if dev.preset_slots() == 0 {
                bail!("{} has no gimbal presets", dev.profile.name);
            }
            match p {
                Preset::List => {
                    for (i, name) in dev.presets()?.into_iter().enumerate() {
                        println!("{}  {}", i + 1, name.as_deref().unwrap_or("(empty)"));
                    }
                }
                Preset::Save { slot, name } => {
                    let name = name.clone().unwrap_or_else(|| format!("Preset{slot}"));
                    dev.save_preset(slot_index(*slot)?, &name)?;
                    println!("saved preset {slot} ({name})");
                }
                Preset::Recall { slot } => dev.recall_preset(slot_index(*slot)?)?,
                Preset::Rename { slot, name } => dev.rename_preset(slot_index(*slot)?, name)?,
            }
        }
        Command::Gimbal { right, up, ms } => {
            let dev = open(&cli)?;
            let start = std::time::Instant::now();
            while start.elapsed().as_millis() < *ms as u128 {
                dev.gimbal_velocity(*right, *up)?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            // Stop twice: a single dropped stop would leave it drifting.
            dev.gimbal_velocity(0.0, 0.0)?;
            std::thread::sleep(std::time::Duration::from_millis(50));
            dev.gimbal_velocity(0.0, 0.0)?;
        }
        Command::Controls => {
            let dev = open(&cli)?;
            for c in dev.node().list_controls()? {
                let value = dev
                    .node()
                    .get_control(c.id)
                    .map_or("-".into(), |v| v.to_string());
                println!(
                    "0x{:08x}  {:<32} {:>8}  [{}..{} step {} def {}] flags=0x{:x}",
                    c.id, c.name, value, c.min, c.max, c.step, c.default, c.flags
                );
                for (v, label) in &c.menu {
                    println!("              {v}: {label}");
                }
            }
        }
        Command::Raw(raw) => {
            let dev = open(&cli)?;
            let node = dev.node();
            match raw {
                Raw::XuProbe => {
                    let found = uvc_xu::probe(node);
                    if found.is_empty() {
                        println!("No Extension Unit controls found.");
                    }
                    for c in found {
                        let caps = format!(
                            "{}{}",
                            if c.info & 1 != 0 { "GET " } else { "" },
                            if c.info & 2 != 0 { "SET" } else { "" }
                        );
                        let cur = uvc_xu::get(node, c.unit, c.selector, UvcQuery::GetCur)
                            .map_or_else(|e| format!("({e})"), |b| hex(&b));
                        println!(
                            "unit {:>2} selector {:>2}  len {:>3}  {:<8} cur: {}",
                            c.unit, c.selector, c.len, caps, cur
                        );
                    }
                }
                Raw::XuGet {
                    unit,
                    selector,
                    query,
                } => {
                    let q = match query.as_str() {
                        "min" => UvcQuery::GetMin,
                        "max" => UvcQuery::GetMax,
                        "def" => UvcQuery::GetDef,
                        "res" => UvcQuery::GetRes,
                        _ => UvcQuery::GetCur,
                    };
                    println!("{}", hex(&uvc_xu::get(node, *unit, *selector, q)?));
                }
                Raw::XuSet {
                    unit,
                    selector,
                    data,
                } => {
                    let bytes = parse_hex(data)?;
                    let len = uvc_xu::len(node, *unit, *selector)? as usize;
                    if bytes.len() != len {
                        bail!(
                            "payload is {} bytes but the control is {len} bytes",
                            bytes.len()
                        );
                    }
                    uvc_xu::set(node, *unit, *selector, &bytes)?;
                    println!("ok");
                }
                Raw::Query { dst, cmd } => {
                    let [dst] = parse_hex(dst)?[..] else {
                        bail!("dst must be one byte, e.g. 04");
                    };
                    let cmd: [u8; 2] = parse_hex(cmd)?
                        .try_into()
                        .map_err(|_| anyhow!("cmd must be two bytes, e.g. 0401"))?;
                    let payload = dev.query(dst, cmd)?;
                    println!(
                        "{}",
                        payload
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                }
            }
        }
    }
    Ok(())
}
