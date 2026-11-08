// SPDX-License-Identifier: AGPL-3.0-or-later
//! The application icon (decision 0542): `cargo xtask icon` renders the master SVG
//! `apps/openchuck-ui/assets/icon/icon.svg` to the committed PNG sizes and writes the
//! Windows icon file (`icon.ico`) and compiled resource (`icon.res`) from them.
//! `cargo xtask icon --check` renders in memory and reports any committed file that differs,
//! so the committed files are always reproducible from the SVG. Nothing here runs at build
//! time: the product build only reads the committed files. The user guide's favicon is
//! written too: `user-docs/favicon/favicon.svg` is a copy of `favicon-adaptive.svg` (the
//! tile-less mark, ink or white by the browser's colour scheme) and `favicon.png`, for
//! browsers without SVG favicons, is the application icon at 32 pixels.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::Findings;

/// The PNG sizes committed beside the SVG, largest first.
pub const SIZES: [u32; 7] = [512, 256, 128, 64, 48, 32, 16];

/// The sizes in the Windows `.ico` and `.res`: Windows picks 16, 32 and 48 for small
/// views and 256 for large ones.
pub const ICO_SIZES: [u32; 5] = [256, 64, 48, 32, 16];

/// The largest difference in any colour or alpha channel, out of 255, allowed between a
/// committed PNG and a fresh rendering. The renderer's anti-aliasing can differ by a step
/// between processors (the tests run on x86-64 and ARM); a change to the drawing moves
/// whole shapes and differs by far more.
pub const PIXEL_TOLERANCE: u8 = 2;

/// Where the icon files live, relative to the workspace root.
pub const ICON_DIR: &str = "apps/openchuck-ui/assets/icon";

/// Where the user guide's favicon files live, relative to the workspace root; the guide's build
/// copies them to the site unchanged (`user-docs/tools/build.py`).
pub const FAVICON_DIR: &str = "user-docs/favicon";

/// Every generated file, by its path relative to the workspace root, with its contents.
pub type Generated = Vec<(String, Vec<u8>)>;

/// Renders `svg` to a square PNG of `size` pixels.
///
/// # Errors
/// If the SVG does not parse or the PNG cannot be encoded.
pub fn render_png(svg: &[u8], size: u32) -> Result<Vec<u8>> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default())
        .context("icon.svg is not a valid SVG")?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)
        .with_context(|| format!("cannot make a {size} x {size} image"))?;
    let view = tree.size();
    #[allow(clippy::cast_precision_loss)]
    let scale = resvg::tiny_skia::Transform::from_scale(
        size as f32 / view.width(),
        size as f32 / view.height(),
    );
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    pixmap.encode_png().context("cannot encode the PNG")
}

/// The width and height in a PNG's header.
///
/// # Errors
/// If the bytes are not a PNG with an `IHDR` chunk first.
pub fn png_size(png: &[u8]) -> Result<(u32, u32)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let be = |at: usize| -> Option<u32> {
        let bytes: [u8; 4] = png.get(at..at + 4)?.try_into().ok()?;
        Some(u32::from_be_bytes(bytes))
    };
    match (png.get(..8), png.get(12..16), be(16), be(20)) {
        (Some(sig), Some(b"IHDR"), Some(w), Some(h)) if sig == SIGNATURE => Ok((w, h)),
        _ => bail!("not a PNG file"),
    }
}

/// The byte a directory entry gives for a dimension: 0 means 256.
fn dimension_byte(size: u32) -> Result<u8> {
    match size {
        256 => Ok(0),
        1..=255 => Ok(u8::try_from(size)?),
        _ => bail!("an icon image must be 1 to 256 pixels, not {size}"),
    }
}

fn u32_len(bytes: &[u8]) -> Result<u32> {
    u32::try_from(bytes.len()).context("an icon image is larger than 4 GiB")
}

/// A Windows `.ico` file holding each PNG as it is (PNG entries, Windows Vista and later).
///
/// # Errors
/// If a PNG is invalid or not square, or there are more than 65535 images.
pub fn ico(pngs: &[Vec<u8>]) -> Result<Vec<u8>> {
    let count = u16::try_from(pngs.len()).context("too many icon images")?;
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + 16 * u32::from(count);
    for png in pngs {
        let (w, h) = png_size(png)?;
        if w != h {
            bail!("an icon image must be square, not {w} x {h}");
        }
        let side = dimension_byte(w)?;
        out.extend_from_slice(&[side, side, 0, 0]); // width, height, colours, reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        let len = u32_len(png)?;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += len;
    }
    for png in pngs {
        out.extend_from_slice(png);
    }
    Ok(out)
}

/// Resource type `RT_ICON`.
const RT_ICON: u16 = 3;
/// Resource type `RT_GROUP_ICON`.
const RT_GROUP_ICON: u16 = 14;
/// Language of the resources: US English, as the resource compiler writes by default.
const LANGUAGE: u16 = 0x0409;

/// One resource in a `.res` file: a header with numeric type and name, then the data
/// padded to four bytes.
fn resource(out: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) -> Result<()> {
    out.extend_from_slice(&u32_len(data)?.to_le_bytes()); // data size
    out.extend_from_slice(&32u32.to_le_bytes()); // header size
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&name.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // data version
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&LANGUAGE.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // version
    out.extend_from_slice(&0u32.to_le_bytes()); // characteristics
    out.extend_from_slice(data);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
    Ok(())
}

/// A compiled Windows resource file (`.res`) holding one icon group (name 1) made of the
/// PNGs (`RT_ICON` names 1, 2, ...). The MSVC linker accepts it as an input file, so no
/// resource compiler is needed to embed the icon in the program.
///
/// # Errors
/// As [`ico`].
pub fn res(pngs: &[Vec<u8>]) -> Result<Vec<u8>> {
    let count = u16::try_from(pngs.len()).context("too many icon images")?;
    let mut out = Vec::new();
    // A `.res` file starts with an empty resource that marks the 32-bit format.
    resource(&mut out, 0, 0, 0, &[])?;
    let mut group = Vec::new();
    group.extend_from_slice(&0u16.to_le_bytes()); // reserved
    group.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    group.extend_from_slice(&count.to_le_bytes());
    for (index, png) in pngs.iter().enumerate() {
        let id = u16::try_from(index + 1)?;
        let (w, h) = png_size(png)?;
        if w != h {
            bail!("an icon image must be square, not {w} x {h}");
        }
        let side = dimension_byte(w)?;
        // Moveable and discardable, as the resource compiler marks icons.
        resource(&mut out, RT_ICON, id, 0x1010, png)?;
        group.extend_from_slice(&[side, side, 0, 0]);
        group.extend_from_slice(&1u16.to_le_bytes());
        group.extend_from_slice(&32u16.to_le_bytes());
        group.extend_from_slice(&u32_len(png)?.to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes());
    }
    // Moveable, pure and discardable, as the resource compiler marks icon groups.
    resource(&mut out, RT_GROUP_ICON, 1, 0x1030, &group)?;
    Ok(out)
}

/// The Windows files made from the PNGs of [`ICO_SIZES`], each found by `png_of(size)`.
fn windows_files(png_of: impl Fn(u32) -> Result<Vec<u8>>) -> Result<Generated> {
    let pngs = ICO_SIZES
        .iter()
        .map(|&size| png_of(size))
        .collect::<Result<Vec<_>>>()?;
    Ok(vec![
        (format!("{ICON_DIR}/icon.ico"), ico(&pngs)?),
        (format!("{ICON_DIR}/icon.res"), res(&pngs)?),
    ])
}

/// Every file `cargo xtask icon` writes, generated from the application icon `svg` and the
/// adaptive favicon `favicon_svg`.
///
/// # Errors
/// If rendering fails.
pub fn generate(svg: &[u8], favicon_svg: &[u8]) -> Result<Generated> {
    let mut files = Generated::new();
    for size in SIZES {
        files.push((
            format!("{ICON_DIR}/icon-{size}.png"),
            render_png(svg, size)?,
        ));
    }
    let rendered = files.clone();
    files.extend(windows_files(|size| {
        let name = format!("{ICON_DIR}/icon-{size}.png");
        rendered
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, png)| png.clone())
            .with_context(|| format!("{size} is not one of the committed sizes"))
    })?);
    files.push((format!("{FAVICON_DIR}/favicon.svg"), favicon_svg.to_vec()));
    files.push((format!("{FAVICON_DIR}/favicon.png"), render_png(svg, 32)?));
    Ok(files)
}

/// Whether two PNGs show the same image within [`PIXEL_TOLERANCE`].
///
/// # Errors
/// If either is not a PNG.
pub fn same_image(a: &[u8], b: &[u8]) -> Result<bool> {
    let a = resvg::tiny_skia::Pixmap::decode_png(a).context("not a PNG file")?;
    let b = resvg::tiny_skia::Pixmap::decode_png(b).context("not a PNG file")?;
    Ok(a.width() == b.width()
        && a.height() == b.height()
        && a.data()
            .iter()
            .zip(b.data())
            .all(|(x, y)| x.abs_diff(*y) <= PIXEL_TOLERANCE))
}

fn icon_dir(root: &Path) -> PathBuf {
    root.join(ICON_DIR)
}

/// The two committed sources: `icon.svg` and `favicon-adaptive.svg`.
fn sources(dir: &Path) -> Result<(Vec<u8>, Vec<u8>)> {
    let read = |name: &str| {
        std::fs::read(dir.join(name))
            .with_context(|| format!("cannot read {}/{name}", dir.display()))
    };
    Ok((read("icon.svg")?, read("favicon-adaptive.svg")?))
}

/// Writes the generated files.
///
/// # Errors
/// If the SVG cannot be read or a file cannot be written.
pub fn write(root: &Path) -> Result<()> {
    let dir = icon_dir(root);
    let (svg, favicon) = sources(&dir)?;
    for (name, bytes) in generate(&svg, &favicon)? {
        let path = root.join(&name);
        std::fs::write(&path, bytes).with_context(|| format!("cannot write {}", path.display()))?;
        println!("wrote {name}");
    }
    Ok(())
}

/// Compares the committed files with what the SVG generates now: each PNG within
/// [`PIXEL_TOLERANCE`], the favicon SVG byte for byte, and the `.ico` and `.res` byte for byte
/// against the committed PNGs they are made of.
///
/// # Errors
/// If the SVG cannot be read or does not render.
pub fn check(root: &Path) -> Result<Findings> {
    let dir = icon_dir(root);
    let (svg, favicon) = sources(&dir)?;
    let mut findings = Findings::new();
    let fix = "run `cargo xtask icon`";
    let mut generated = generate(&svg, &favicon)?;
    generated.retain(|(name, _)| !name.ends_with(".ico") && !name.ends_with(".res"));
    for (name, bytes) in generated {
        let Ok(committed) = std::fs::read(root.join(&name)) else {
            findings.push(format!("{name} is missing; {fix}"));
            continue;
        };
        let same = if name.ends_with(".png") {
            same_image(&committed, &bytes).unwrap_or(false)
        } else {
            committed == bytes
        };
        if !same {
            findings.push(format!(
                "{name} differs from what {ICON_DIR}/icon.svg gives; {fix}"
            ));
        }
    }
    if findings.is_empty() {
        let committed_png = |size: u32| {
            let path = dir.join(format!("icon-{size}.png"));
            std::fs::read(&path).with_context(|| format!("cannot read {}", path.display()))
        };
        for (name, bytes) in windows_files(committed_png)? {
            if std::fs::read(root.join(&name)).ok().as_deref() != Some(bytes.as_slice()) {
                findings.push(format!("{name} is not what the committed PNGs give; {fix}"));
            }
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><rect x="0" y="0" width="256" height="512" fill="#FF0000"/></svg>"##;

    fn le16(b: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([b[at], b[at + 1]])
    }
    fn le32(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
    }

    #[test]
    fn icon_render_gives_a_png_of_the_asked_size_with_the_drawing_scaled() {
        let png = render_png(SQUARE.as_bytes(), 16).unwrap();
        assert_eq!(png_size(&png).unwrap(), (16, 16));
        let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        // The left half is red and opaque, the right half transparent.
        let left = pixmap.pixel(3, 8).unwrap();
        let right = pixmap.pixel(12, 8).unwrap();
        assert_eq!((left.red(), left.alpha()), (255, 255));
        assert_eq!(right.alpha(), 0);
    }

    #[test]
    fn icon_render_is_deterministic() {
        let a = render_png(SQUARE.as_bytes(), 64).unwrap();
        let b = render_png(SQUARE.as_bytes(), 64).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn icon_invalid_svg_is_an_error_not_a_panic() {
        assert!(render_png(b"not svg", 16).is_err());
        assert!(png_size(b"not a png at all, long enough").is_err());
        assert!(ico(&[b"short".to_vec()]).is_err());
    }

    #[test]
    fn icon_png_size_reads_the_header() {
        let png = render_png(SQUARE.as_bytes(), 48).unwrap();
        assert_eq!(png_size(&png).unwrap(), (48, 48));
    }

    #[test]
    fn icon_ico_has_a_directory_entry_per_png_pointing_at_its_bytes() {
        let pngs: Vec<Vec<u8>> = [256, 32, 16]
            .iter()
            .map(|&s| render_png(SQUARE.as_bytes(), s).unwrap())
            .collect();
        let file = ico(&pngs).unwrap();
        assert_eq!(le16(&file, 0), 0);
        assert_eq!(le16(&file, 2), 1);
        assert_eq!(le16(&file, 4), 3);
        for (i, (png, side)) in pngs.iter().zip([0u8, 32, 16]).enumerate() {
            let e = 6 + 16 * i;
            assert_eq!(file[e], side, "width byte of entry {i}");
            assert_eq!(file[e + 1], side, "height byte of entry {i}");
            assert_eq!(le16(&file, e + 4), 1);
            assert_eq!(le16(&file, e + 6), 32);
            let len = le32(&file, e + 8) as usize;
            let at = le32(&file, e + 12) as usize;
            assert_eq!(len, png.len());
            assert_eq!(&file[at..at + len], png.as_slice());
        }
        assert_eq!(
            file.len(),
            6 + 16 * 3 + pngs.iter().map(Vec::len).sum::<usize>()
        );
    }

    #[test]
    fn icon_ico_rejects_a_non_square_or_oversized_image() {
        let wide = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2 1"/>"##;
        let tree = resvg::usvg::Tree::from_data(wide.as_bytes(), &resvg::usvg::Options::default())
            .unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(4, 2).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::default(),
            &mut pixmap.as_mut(),
        );
        let png = pixmap.encode_png().unwrap();
        assert!(ico(std::slice::from_ref(&png)).is_err());
        assert!(res(&[png]).is_err());
        let big = render_png(SQUARE.as_bytes(), 512).unwrap();
        assert!(ico(&[big]).is_err());
    }

    /// Walks a `.res` file: (type, name, language, flags, data) per resource.
    fn resources(file: &[u8]) -> Vec<(u16, u16, u16, u16, Vec<u8>)> {
        let mut out = Vec::new();
        let mut at = 0;
        while at < file.len() {
            let size = le32(file, at) as usize;
            let header = le32(file, at + 4) as usize;
            assert_eq!(header, 32);
            assert_eq!(le16(file, at + 8), 0xFFFF);
            assert_eq!(le16(file, at + 12), 0xFFFF);
            let kind = le16(file, at + 10);
            let name = le16(file, at + 14);
            let flags = le16(file, at + 20);
            let lang = le16(file, at + 22);
            out.push((
                kind,
                name,
                lang,
                flags,
                file[at + 32..at + 32 + size].to_vec(),
            ));
            at += 32 + size;
            at = at.div_ceil(4) * 4;
        }
        assert_eq!(at, file.len());
        out
    }

    #[test]
    fn icon_res_holds_an_empty_first_resource_one_rt_icon_per_png_and_one_group() {
        let pngs: Vec<Vec<u8>> = [256, 48, 16]
            .iter()
            .map(|&s| render_png(SQUARE.as_bytes(), s).unwrap())
            .collect();
        let file = res(&pngs).unwrap();
        let all = resources(&file);
        assert_eq!(all.len(), 1 + 3 + 1);
        assert_eq!((all[0].0, all[0].1, all[0].4.len()), (0, 0, 0));
        for (i, png) in pngs.iter().enumerate() {
            let (kind, name, lang, flags, data) = &all[1 + i];
            assert_eq!(*kind, RT_ICON);
            assert_eq!(usize::from(*name), i + 1);
            assert_eq!(*lang, LANGUAGE);
            assert_eq!(*flags, 0x1010);
            assert_eq!(data, png);
        }
        let (kind, name, _, flags, group) = &all[4];
        assert_eq!((*kind, *name, *flags), (RT_GROUP_ICON, 1, 0x1030));
        assert_eq!((le16(group, 0), le16(group, 2), le16(group, 4)), (0, 1, 3));
        assert_eq!(group.len(), 6 + 14 * 3);
        for (i, (png, side)) in pngs.iter().zip([0u8, 48, 16]).enumerate() {
            let e = 6 + 14 * i;
            assert_eq!((group[e], group[e + 1]), (side, side));
            assert_eq!(le16(group, e + 4), 1);
            assert_eq!(le16(group, e + 6), 32);
            assert_eq!(le32(group, e + 8) as usize, png.len());
            assert_eq!(usize::from(le16(group, e + 12)), i + 1);
        }
    }

    #[test]
    fn icon_generate_makes_every_committed_size_and_the_windows_files() {
        let favicon = b"<svg/>";
        let files = generate(SQUARE.as_bytes(), favicon).unwrap();
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "apps/openchuck-ui/assets/icon/icon-512.png",
                "apps/openchuck-ui/assets/icon/icon-256.png",
                "apps/openchuck-ui/assets/icon/icon-128.png",
                "apps/openchuck-ui/assets/icon/icon-64.png",
                "apps/openchuck-ui/assets/icon/icon-48.png",
                "apps/openchuck-ui/assets/icon/icon-32.png",
                "apps/openchuck-ui/assets/icon/icon-16.png",
                "apps/openchuck-ui/assets/icon/icon.ico",
                "apps/openchuck-ui/assets/icon/icon.res",
                "user-docs/favicon/favicon.svg",
                "user-docs/favicon/favicon.png",
            ]
        );
        for ((name, bytes), size) in files[..7].iter().zip(SIZES) {
            assert_eq!(png_size(bytes).unwrap(), (size, size), "{name}");
        }
        let ico_file = &files[7].1;
        assert_eq!(le16(ico_file, 4), u16::try_from(ICO_SIZES.len()).unwrap());
        assert_eq!(files[9].1, favicon);
        assert_eq!(png_size(&files[10].1).unwrap(), (32, 32));
    }

    #[test]
    fn icon_same_image_allows_an_anti_aliasing_step_but_not_a_changed_drawing() {
        let png = render_png(SQUARE.as_bytes(), 32).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        // One channel of one pixel off by the tolerance: the same image.
        let data = pixmap.data_mut();
        data[0] = data[0].saturating_sub(PIXEL_TOLERANCE);
        let nudged = pixmap.encode_png().unwrap();
        assert!(same_image(&png, &nudged).unwrap());
        // A changed drawing: the red half moved.
        let moved = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><rect x="32" y="0" width="256" height="512" fill="#FF0000"/></svg>"##;
        let other = render_png(moved.as_bytes(), 32).unwrap();
        assert!(!same_image(&png, &other).unwrap());
        // A different size is never the same image.
        let small = render_png(SQUARE.as_bytes(), 16).unwrap();
        assert!(!same_image(&png, &small).unwrap());
        assert!(same_image(&png, b"not a png").is_err());
    }

    /// The macOS bundle's icon (decision 0541) is the committed 512-pixel PNG, and
    /// `cargo xtask macos-app` accepts it as its `.icns` image (decision 0542).
    #[test]
    fn icon_the_bundle_takes_the_committed_512_pixel_png() {
        let root = crate::workspace_root().unwrap();
        let png = std::fs::read(root.join(ICON_DIR).join("icon-512.png")).unwrap();
        let icns = crate::macos_app::icns(&png).expect("the bundle accepts the icon");
        assert_eq!(&icns[..4], b"icns");
    }

    /// The committed icon files are what `cargo xtask icon` makes from the committed SVG
    /// (decision 0542: reproducible from the SVG, no network or renderer at build time).
    #[test]
    fn icon_committed_files_match_the_svg() {
        let root = crate::workspace_root().unwrap();
        if !root.join(ICON_DIR).join("icon.svg").exists() {
            panic!("{ICON_DIR}/icon.svg is missing");
        }
        assert_eq!(check(&root).unwrap(), Findings::new());
    }
}
