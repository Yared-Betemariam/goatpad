use image::{DynamicImage, ImageFormat, RgbaImage, imageops::FilterType};
use std::{error::Error, fs, io::Cursor, path::Path};

const SOURCE_ICON: &str = "assets/raw-icon.png";
const WINDOWS_ICON: &str = "assets/icon.ico";
const RUNTIME_ICON: &str = "assets/icon.rgba";
const RUNTIME_ICON_SIZE: u32 = 64;
const WINDOWS_ICON_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

fn main() {
    println!("cargo:rerun-if-changed={SOURCE_ICON}");

    generate_icons().expect("failed to generate Goatpad icons from assets/raw-icon.png");

    #[cfg(target_os = "windows")]
    {
        let mut resources = winres::WindowsResource::new();
        resources.set_icon(WINDOWS_ICON);
        resources
            .compile()
            .expect("failed to compile the Goatpad Windows resources");
    }
}

fn generate_icons() -> Result<(), Box<dyn Error>> {
    let source_bytes = fs::read(SOURCE_ICON)?;
    let source = image::load_from_memory_with_format(&source_bytes, ImageFormat::Png)?;

    let runtime_icon = fit_to_square(&source, RUNTIME_ICON_SIZE);
    write_if_changed(Path::new(RUNTIME_ICON), runtime_icon.as_raw())?;

    let mut frames = Vec::with_capacity(WINDOWS_ICON_SIZES.len());
    for size in WINDOWS_ICON_SIZES {
        let icon = fit_to_square(&source, size);
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(icon).write_to(&mut encoded, ImageFormat::Png)?;
        frames.push((size, encoded.into_inner()));
    }

    let ico = encode_ico(&frames);
    write_if_changed(Path::new(WINDOWS_ICON), &ico)?;
    Ok(())
}

fn fit_to_square(source: &DynamicImage, size: u32) -> RgbaImage {
    let resized = source.resize(size, size, FilterType::Lanczos3).to_rgba8();
    let mut canvas = RgbaImage::new(size, size);
    let x = i64::from((size - resized.width()) / 2);
    let y = i64::from((size - resized.height()) / 2);
    image::imageops::overlay(&mut canvas, &resized, x, y);
    canvas
}

fn encode_ico(frames: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let directory_size = 6 + frames.len() * 16;
    let data_size = frames.iter().map(|(_, png)| png.len()).sum::<usize>();
    let mut ico = Vec::with_capacity(directory_size + data_size);

    ico.extend_from_slice(&0_u16.to_le_bytes());
    ico.extend_from_slice(&1_u16.to_le_bytes());
    ico.extend_from_slice(&(frames.len() as u16).to_le_bytes());

    let mut image_offset = directory_size as u32;
    for (size, png) in frames {
        ico.push(if *size == 256 { 0 } else { *size as u8 });
        ico.push(if *size == 256 { 0 } else { *size as u8 });
        ico.push(0);
        ico.push(0);
        ico.extend_from_slice(&1_u16.to_le_bytes());
        ico.extend_from_slice(&32_u16.to_le_bytes());
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&image_offset.to_le_bytes());
        image_offset += png.len() as u32;
    }

    for (_, png) in frames {
        ico.extend_from_slice(png);
    }

    ico
}

fn write_if_changed(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let is_current = fs::read(path)
        .map(|existing| existing == contents)
        .unwrap_or(false);
    if !is_current {
        fs::write(path, contents)?;
    }
    Ok(())
}
