use anyhow::{Context, Result};
use base64::Engine as _;
use ratatui::{layout::Rect, style::Color};
use std::{
    fs::File,
    io::{Read, Write as _},
    path::Path,
};

pub(super) fn place_terminal_image_with_kitty_protocol(
    path: &Path,
    area: Rect,
    excluded: &[Rect],
) -> Result<Vec<u8>> {
    let id = kitty_image_id();
    let mut out = build_kitty_upload_sequence(path, id, area)?;
    out.extend(build_kitty_placeholder_sequence(id, area, excluded));
    Ok(maybe_wrap_kitty_apcs_for_tmux(out))
}

pub(super) fn clear_terminal_images_with_kitty_protocol() -> Result<Vec<u8>> {
    Ok(maybe_wrap_kitty_apcs_for_tmux(
        build_kitty_clear_sequence().as_bytes().to_vec(),
    ))
}

/// Inside tmux, wrap each Kitty APC sequence in the tmux DCS passthrough
/// envelope so tmux relays it to the host terminal. CSI sequences and the
/// Unicode placeholder characters are emitted unchanged. Outside tmux the
/// input is returned untouched.
///
/// Background: tmux's `allow-passthrough on` only forwards DCS sequences with
/// the `\ePtmux;<seq>\e\\` envelope. Raw APC sequences (`\e_G…\e\\`, used by
/// the Kitty Graphics Protocol) are swallowed by tmux and never reach the
/// host terminal — so the host terminal never registers the image and the
/// Unicode placeholders render as empty cells.
fn maybe_wrap_kitty_apcs_for_tmux(buf: Vec<u8>) -> Vec<u8> {
    if std::env::var_os("TMUX").is_none() {
        return buf;
    }
    wrap_kitty_apcs_for_tmux(&buf)
}

fn wrap_kitty_apcs_for_tmux(buf: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len() + buf.len() / 4);
    let mut i = 0;
    while i < buf.len() {
        if buf.len() - i >= 3
            && &buf[i..i + 3] == b"\x1b_G"
            && let Some(rel) = buf[i + 3..].iter().position(|&b| b == 0x1b)
            && buf.get(i + 3 + rel + 1) == Some(&b'\\')
        {
            let body_end = i + 3 + rel;
            out.extend_from_slice(b"\x1bPtmux;\x1b\x1b_G");
            out.extend_from_slice(&buf[i + 3..body_end]);
            out.extend_from_slice(b"\x1b\x1b\\\x1b\\");
            i = body_end + 2;
            continue;
        }
        out.push(buf[i]);
        i += 1;
    }
    out
}

fn build_kitty_upload_sequence(path: &Path, id: u32, area: Rect) -> Result<Vec<u8>> {
    // Send PNG bytes inline instead of handing Kitty/Ghostty a filesystem path.
    // JPEG/WebP/GIF/SVG previews are first rendered into a cache PNG and then
    // displayed via Kitty. With the old `t=f` upload, the terminal had to
    // reopen that freshly-written cache file on its own, and because we also
    // suppress Kitty failure replies (`q=2`) the app could not tell when that
    // read failed or raced. Inlining the PNG data removes that extra file-open
    // step entirely.
    let mut file = File::open(path)
        .with_context(|| format!("failed to open kitty preview image {}", path.display()))?;
    let total = file
        .metadata()
        .with_context(|| format!("failed to stat kitty preview image {}", path.display()))?
        .len() as usize;
    if total == 0 {
        anyhow::bail!("kitty preview image {} is empty", path.display());
    }

    let mut sent = 0usize;
    let mut chunk = vec![0u8; 3 * 4096 / 4];
    let mut out = Vec::new();
    while sent < total {
        let remaining = total.saturating_sub(sent);
        let chunk_len = remaining.min(chunk.len());
        file.read_exact(&mut chunk[..chunk_len])
            .with_context(|| format!("failed to read kitty preview image {}", path.display()))?;
        sent += chunk_len;
        let more = sent < total;
        let payload = base64::engine::general_purpose::STANDARD.encode(&chunk[..chunk_len]);
        if sent == chunk_len {
            write!(
                out,
                "\u{1b}_Ga=T,q=2,f=100,U=1,i={id},p=1,c={},r={},C=1,m={};{payload}\u{1b}\\",
                area.width.max(1),
                area.height.max(1),
                if more { 1 } else { 0 },
            )?;
        } else {
            write!(
                out,
                "\u{1b}_Gm={};{payload}\u{1b}\\",
                if more { 1 } else { 0 },
            )?;
        }
    }
    Ok(out)
}

fn build_kitty_placeholder_sequence(id: u32, area: Rect, excluded: &[Rect]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(usize::from(area.width) * usize::from(area.height) * 8 + 32);
    let (r, g, b) = ((id >> 16) & 0xff, (id >> 8) & 0xff, id & 0xff);
    match crate::ui::theme::palette().panel {
        Color::Rgb(bg_r, bg_g, bg_b) => {
            let _ = write!(
                buf,
                "\x1b[38;2;{r};{g};{b};48;2;{bg_r};{bg_g};{bg_b};58;2;0;0;1m"
            );
        }
        _ => {
            let _ = write!(buf, "\x1b[38;2;{r};{g};{b};58;2;0;0;1m");
        }
    }
    for y in 0..area.height {
        let abs_row = area.y.saturating_add(y);
        let dy = usize::from(y).min(DIACRITICS.len() - 1);
        let mut need_pos = true;
        for x in 0..area.width {
            let abs_col = area.x.saturating_add(x);
            if excluded
                .iter()
                .any(|r| kitty_cell_in_rect(abs_col, abs_row, r))
            {
                need_pos = true;
                continue;
            }
            if need_pos {
                let _ = write!(
                    buf,
                    "\x1b[{};{}H",
                    abs_row.saturating_add(1),
                    abs_col.saturating_add(1)
                );
                need_pos = false;
            }
            let dx = usize::from(x).min(DIACRITICS.len() - 1);
            let _ = write!(buf, "\u{10EEEE}{}{}", DIACRITICS[dy], DIACRITICS[dx]);
        }
    }
    let _ = write!(buf, "\x1b[0m");
    buf
}

fn build_kitty_clear_sequence() -> &'static str {
    "\u{1b}_Ga=d,d=A,q=2\u{1b}\\"
}

fn kitty_cell_in_rect(col: u16, row: u16, rect: &Rect) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn kitty_image_id() -> u32 {
    std::process::id() % (0xff_ffff + 1)
}

#[rustfmt::skip]
static DIACRITICS: [char; 297] = [
    '\u{0305}', '\u{030D}', '\u{030E}', '\u{0310}', '\u{0312}', '\u{033D}', '\u{033E}',
    '\u{033F}', '\u{0346}', '\u{034A}', '\u{034B}', '\u{034C}', '\u{0350}', '\u{0351}',
    '\u{0352}', '\u{0357}', '\u{035B}', '\u{0363}', '\u{0364}', '\u{0365}', '\u{0366}',
    '\u{0367}', '\u{0368}', '\u{0369}', '\u{036A}', '\u{036B}', '\u{036C}', '\u{036D}',
    '\u{036E}', '\u{036F}', '\u{0483}', '\u{0484}', '\u{0485}', '\u{0486}', '\u{0487}',
    '\u{0592}', '\u{0593}', '\u{0594}', '\u{0595}', '\u{0597}', '\u{0598}', '\u{0599}',
    '\u{059C}', '\u{059D}', '\u{059E}', '\u{059F}', '\u{05A0}', '\u{05A1}', '\u{05A8}',
    '\u{05A9}', '\u{05AB}', '\u{05AC}', '\u{05AF}', '\u{05C4}', '\u{0610}', '\u{0611}',
    '\u{0612}', '\u{0613}', '\u{0614}', '\u{0615}', '\u{0616}', '\u{0617}', '\u{0657}',
    '\u{0658}', '\u{0659}', '\u{065A}', '\u{065B}', '\u{065D}', '\u{065E}', '\u{06D6}',
    '\u{06D7}', '\u{06D8}', '\u{06D9}', '\u{06DA}', '\u{06DB}', '\u{06DC}', '\u{06DF}',
    '\u{06E0}', '\u{06E1}', '\u{06E2}', '\u{06E4}', '\u{06E7}', '\u{06E8}', '\u{06EB}',
    '\u{06EC}', '\u{0730}', '\u{0732}', '\u{0733}', '\u{0735}', '\u{0736}', '\u{073A}',
    '\u{073D}', '\u{073F}', '\u{0740}', '\u{0741}', '\u{0743}', '\u{0745}', '\u{0747}',
    '\u{0749}', '\u{074A}', '\u{07EB}', '\u{07EC}', '\u{07ED}', '\u{07EE}', '\u{07EF}',
    '\u{07F0}', '\u{07F1}', '\u{07F3}', '\u{0816}', '\u{0817}', '\u{0818}', '\u{0819}',
    '\u{081B}', '\u{081C}', '\u{081D}', '\u{081E}', '\u{081F}', '\u{0820}', '\u{0821}',
    '\u{0822}', '\u{0823}', '\u{0825}', '\u{0826}', '\u{0827}', '\u{0829}', '\u{082A}',
    '\u{082B}', '\u{082C}', '\u{082D}', '\u{0951}', '\u{0953}', '\u{0954}', '\u{0F82}',
    '\u{0F83}', '\u{0F86}', '\u{0F87}', '\u{135D}', '\u{135E}', '\u{135F}', '\u{17DD}',
    '\u{193A}', '\u{1A17}', '\u{1A75}', '\u{1A76}', '\u{1A77}', '\u{1A78}', '\u{1A79}',
    '\u{1A7A}', '\u{1A7B}', '\u{1A7C}', '\u{1B6B}', '\u{1B6D}', '\u{1B6E}', '\u{1B6F}',
    '\u{1B70}', '\u{1B71}', '\u{1B72}', '\u{1B73}', '\u{1CD0}', '\u{1CD1}', '\u{1CD2}',
    '\u{1CDA}', '\u{1CDB}', '\u{1CE0}', '\u{1DC0}', '\u{1DC1}', '\u{1DC3}', '\u{1DC4}',
    '\u{1DC5}', '\u{1DC6}', '\u{1DC7}', '\u{1DC8}', '\u{1DC9}', '\u{1DCB}', '\u{1DCC}',
    '\u{1DD1}', '\u{1DD2}', '\u{1DD3}', '\u{1DD4}', '\u{1DD5}', '\u{1DD6}', '\u{1DD7}',
    '\u{1DD8}', '\u{1DD9}', '\u{1DDA}', '\u{1DDB}', '\u{1DDC}', '\u{1DDD}', '\u{1DDE}',
    '\u{1DDF}', '\u{1DE0}', '\u{1DE1}', '\u{1DE2}', '\u{1DE3}', '\u{1DE4}', '\u{1DE5}',
    '\u{1DE6}', '\u{1DFE}', '\u{20D0}', '\u{20D1}', '\u{20D4}', '\u{20D5}', '\u{20D6}',
    '\u{20D7}', '\u{20DB}', '\u{20DC}', '\u{20E1}', '\u{20E7}', '\u{20E9}', '\u{20F0}',
    '\u{2CEF}', '\u{2CF0}', '\u{2CF1}', '\u{2DE0}', '\u{2DE1}', '\u{2DE2}', '\u{2DE3}',
    '\u{2DE4}', '\u{2DE5}', '\u{2DE6}', '\u{2DE7}', '\u{2DE8}', '\u{2DE9}', '\u{2DEA}',
    '\u{2DEB}', '\u{2DEC}', '\u{2DED}', '\u{2DEE}', '\u{2DEF}', '\u{2DF0}', '\u{2DF1}',
    '\u{2DF2}', '\u{2DF3}', '\u{2DF4}', '\u{2DF5}', '\u{2DF6}', '\u{2DF7}', '\u{2DF8}',
    '\u{2DF9}', '\u{2DFA}', '\u{2DFB}', '\u{2DFC}', '\u{2DFD}', '\u{2DFE}', '\u{2DFF}',
    '\u{A66F}', '\u{A67C}', '\u{A67D}', '\u{A6F0}', '\u{A6F1}', '\u{A8E0}', '\u{A8E1}',
    '\u{A8E2}', '\u{A8E3}', '\u{A8E4}', '\u{A8E5}', '\u{A8E6}', '\u{A8E7}', '\u{A8E8}',
    '\u{A8E9}', '\u{A8EA}', '\u{A8EB}', '\u{A8EC}', '\u{A8ED}', '\u{A8EE}', '\u{A8EF}',
    '\u{A8F0}', '\u{A8F1}', '\u{AAB0}', '\u{AAB2}', '\u{AAB3}', '\u{AAB7}', '\u{AAB8}',
    '\u{AABE}', '\u{AABF}', '\u{AAC1}', '\u{FE20}', '\u{FE21}', '\u{FE22}', '\u{FE23}',
    '\u{FE24}', '\u{FE25}', '\u{FE26}', '\u{10A0F}', '\u{10A38}', '\u{1D185}', '\u{1D186}',
    '\u{1D187}', '\u{1D188}', '\u{1D189}', '\u{1D1AA}', '\u{1D1AB}', '\u{1D1AC}', '\u{1D1AD}',
    '\u{1D242}', '\u{1D243}', '\u{1D244}',
];

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use image::ImageFormat;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-inline-image-{label}-{unique}"))
    }

    fn write_test_raster_image(path: &Path, format: ImageFormat, width: u32, height: u32) {
        let image =
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(width, height, |x, y| {
                image::Rgba([(x % 255) as u8, (y % 255) as u8, 0x80, 0xff])
            }));
        image
            .save_with_format(path, format)
            .expect("test raster image should save");
    }

    #[test]
    fn build_kitty_upload_sequence_uses_unicode_placeholder_mode() {
        let root = temp_root("kitty-upload-sequence");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("demo.pdf-preview.png");
        write_test_raster_image(&path, ImageFormat::Png, 24, 16);
        let payload = fs::read(&path).expect("png payload should exist");
        let id = 42_u32;
        let area = Rect {
            x: 10,
            y: 4,
            width: 30,
            height: 20,
        };

        let sequence = String::from_utf8(
            build_kitty_upload_sequence(&path, id, area)
                .expect("kitty upload sequence should build"),
        )
        .expect("kitty upload sequence should be utf8");

        assert!(sequence.starts_with("\u{1b}_G"));
        assert!(sequence.contains("a=T"));
        assert!(sequence.contains("q=2"));
        assert!(sequence.contains("U=1"));
        assert!(sequence.contains(&format!("i={id}")));
        assert!(sequence.contains("p=1"));
        assert!(sequence.contains("c=30"));
        assert!(sequence.contains("r=20"));
        assert!(sequence.contains("C=1"));
        assert!(sequence.contains("m=0"));
        assert!(!sequence.contains("t=f"));
        assert!(sequence.contains(&BASE64_STANDARD.encode(payload)));
        assert!(sequence.ends_with("\u{1b}\\"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn kitty_placeholder_sequence_sets_panel_background_for_transparency() {
        let sequence = String::from_utf8(build_kitty_placeholder_sequence(
            42,
            Rect {
                x: 1,
                y: 2,
                width: 2,
                height: 2,
            },
            &[],
        ))
        .expect("placeholder sequence should be utf8");

        assert!(sequence.contains("[38;2;"));
        assert!(sequence.contains(";48;2;"));
        assert!(sequence.contains(";58;2;0;0;1m"));
    }

    #[test]
    fn build_kitty_clear_sequence_deletes_visible_images() {
        assert_eq!(build_kitty_clear_sequence(), "\u{1b}_Ga=d,d=A,q=2\u{1b}\\");
    }

    #[test]
    fn wrap_kitty_apcs_for_tmux_envelopes_each_apc_and_leaves_csi_alone() {
        let input =
            b"\x1b_Ga=T,i=1,c=2,r=2;AAAA\x1b\\\x1b[5;10H\xf4\x8e\xbb\xae\x1b_Gm=0;BBBB\x1b\\";
        let out = wrap_kitty_apcs_for_tmux(input);
        let expected: &[u8] = b"\x1bPtmux;\x1b\x1b_Ga=T,i=1,c=2,r=2;AAAA\x1b\x1b\\\x1b\\\x1b[5;10H\xf4\x8e\xbb\xae\x1bPtmux;\x1b\x1b_Gm=0;BBBB\x1b\x1b\\\x1b\\";
        assert_eq!(out, expected);
    }

    #[test]
    fn wrap_kitty_apcs_for_tmux_is_noop_without_apcs() {
        let input = b"\x1b[5;10Hhello\x1b[0m";
        let out = wrap_kitty_apcs_for_tmux(input);
        assert_eq!(out, input);
    }
}
