use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Config {
    #[arg(long)]
    pub window: String,
    #[arg(long)]
    pub output: PathBuf,
    #[arg(skip)]
    pub display: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Lsb,
    Msb,
}

pub struct Layout {
    pub width: u16,
    pub height: u16,
    pub depth: u8,
    pub bits: u8,
    pub pad: u8,
    pub order: Order,
    pub masks: [u32; 3],
}

pub fn ppm(layout: &Layout, data: &[u8]) -> crate::Result<Vec<u8>> {
    if !matches!(layout.depth, 24 | 32) || layout.bits != 32 {
        return Err(format!(
            "unsupported visual: depth {} at {} bits per pixel",
            layout.depth, layout.bits
        ));
    }
    if layout.masks != [0x00ff_0000, 0x0000_ff00, 0x0000_00ff] {
        return Err(format!("unsupported visual masks: {:08x?}", layout.masks));
    }
    let pad = usize::from(layout.pad.max(8)) / 8;
    let width = usize::from(layout.width);
    let height = usize::from(layout.height);
    let stride = (width * 4).div_ceil(pad) * pad;
    if data.len() < stride * height {
        return Err(format!(
            "short image: {} bytes for {width}x{height} stride {stride}",
            data.len()
        ));
    }
    let mut output = format!("P6\n{width} {height}\n255\n").into_bytes();
    output.reserve(width * height * 3);
    for row in 0..height {
        for column in 0..width {
            let at = row * stride + column * 4;
            let bytes = [data[at], data[at + 1], data[at + 2], data[at + 3]];
            let pixel = match layout.order {
                Order::Lsb => u32::from_le_bytes(bytes),
                Order::Msb => u32::from_be_bytes(bytes),
            };
            output.extend_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
        }
    }
    Ok(output)
}

#[cfg(not(target_os = "linux"))]
pub fn run(config: &Config) -> crate::Result<Value> {
    let _ = config;
    Err("snap requires an X11 display on Linux".to_owned())
}

#[cfg(target_os = "linux")]
pub fn run(config: &Config) -> crate::Result<Value> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, ImageOrder};

    let display = config
        .display
        .clone()
        .ok_or("DISPLAY is not set; snap needs a reachable X11 display")?;
    let (connection, screen) =
        x11rb::connect(None).map_err(|error| format!("connect {display}: {error}"))?;
    let setup = connection.setup();
    let root = setup.roots[screen].root;
    let window = match config.window.as_str() {
        "root" => root,
        text => parse(text)?,
    };
    let geometry = connection
        .get_geometry(window)
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| format!("window 0x{window:x}: {error}"))?;
    let attributes = connection
        .get_window_attributes(window)
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| format!("window 0x{window:x}: {error}"))?;
    let image = connection
        .get_image(
            ImageFormat::Z_PIXMAP,
            window,
            0,
            0,
            geometry.width,
            geometry.height,
            u32::MAX,
        )
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| format!("GetImage 0x{window:x}: {error}"))?;
    let format = setup
        .pixmap_formats
        .iter()
        .find(|format| format.depth == image.depth)
        .ok_or_else(|| format!("no pixmap format for depth {}", image.depth))?;
    let visual = setup
        .roots
        .iter()
        .flat_map(|screen| &screen.allowed_depths)
        .flat_map(|depth| &depth.visuals)
        .find(|visual| visual.visual_id == image.visual || visual.visual_id == attributes.visual)
        .ok_or_else(|| format!("visual 0x{:x} not found", image.visual))?;
    let layout = Layout {
        width: geometry.width,
        height: geometry.height,
        depth: image.depth,
        bits: format.bits_per_pixel,
        pad: format.scanline_pad,
        order: if setup.image_byte_order == ImageOrder::MSB_FIRST {
            Order::Msb
        } else {
            Order::Lsb
        },
        masks: [visual.red_mask, visual.green_mask, visual.blue_mask],
    };
    let frame = ppm(&layout, &image.data)?;
    let vendor = String::from_utf8_lossy(&setup.vendor).into_owned();
    let output = crate::Output::prepare(&config.output)?;
    let path = output.join("frame.ppm");
    std::fs::write(&path, &frame).map_err(|error| format!("{}: {error}", path.display()))?;
    let receipt = json!({
        "kind": "x11-getimage",
        "display": display,
        "window": format!("0x{window:x}"),
        "root": window == root,
        "width": geometry.width,
        "height": geometry.height,
        "depth": image.depth,
        "visual": format!("0x{:x}", visual.visual_id),
        "vendor": vendor,
        "release": setup.release_number,
        "physical": false,
        "evidence": "native-window-unqualified",
        "frame": "frame.ppm",
        "sha256": crate::digest(&frame),
    });
    crate::write(&output.join("receipt.json"), &receipt)?;
    Ok(receipt)
}

#[cfg(target_os = "linux")]
fn parse(text: &str) -> crate::Result<u32> {
    let parsed = match text.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => text.parse(),
    };
    parsed.map_err(|_| format!("window must be root, decimal or 0x hex: {text}"))
}
