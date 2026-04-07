use super::TerminalWindowSize;
use crossterm::terminal;
use std::sync::OnceLock;

pub(super) fn query_terminal_window_size() -> Option<TerminalWindowSize> {
    let terminal_size = terminal::window_size().ok();
    let (cells_width, cells_height) = terminal_size
        .as_ref()
        .map(|size| (size.columns, size.rows))
        .or_else(|| terminal::size().ok())?;
    let (pixels_width, pixels_height) = terminal_size
        .as_ref()
        .and_then(|size| {
            let width = u32::from(size.width);
            let height = u32::from(size.height);
            (width > 0 && height > 0).then_some((width, height))
        })
        .or_else(|| query_pixels_from_cell_size(cells_width, cells_height))
        .unwrap_or_else(|| fallback_window_size_pixels(cells_width, cells_height));
    Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width,
        pixels_height,
    })
}

/// Ask the terminal for its per-cell pixel size via `CSI 16 t` and derive
/// total pixel dimensions from the current cell count.
///
/// The response (`CSI 6 ; <cell_h> ; <cell_w> t`) is read from `/dev/tty`
/// with a 100 ms timeout so we never block on terminals that ignore the query.
/// The per-cell result is cached in a `OnceLock` so the tty is only touched
/// once per process — subsequent resize events reuse the cached cell size with
/// updated cell counts.
#[cfg(unix)]
fn query_pixels_from_cell_size(cells_width: u16, cells_height: u16) -> Option<(u32, u32)> {
    static CELL_PX: OnceLock<Option<(u32, u32)>> = OnceLock::new();
    let (cell_w, cell_h) = (*CELL_PX.get_or_init(query_cell_pixel_size_from_terminal))?;
    Some((
        cell_w * u32::from(cells_width.max(1)),
        cell_h * u32::from(cells_height.max(1)),
    ))
}

#[cfg(not(unix))]
fn query_pixels_from_cell_size(_cells_width: u16, _cells_height: u16) -> Option<(u32, u32)> {
    None
}

/// Send `CSI 16 t` to `/dev/tty` and read the `CSI 6 ; <ph> ; <pw> t`
/// response, returning `(cell_width_px, cell_height_px)`.
#[cfg(unix)]
fn query_cell_pixel_size_from_terminal() -> Option<(u32, u32)> {
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::os::unix::io::AsRawFd;

    let mut tty = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()?;

    tty.write_all(b"\x1b[16t").ok()?;
    tty.flush().ok()?;

    let tty_fd = tty.as_raw_fd();
    // Wait up to 100 ms for the terminal to respond.
    let mut tv = libc::timeval {
        tv_sec: 0,
        tv_usec: 100_000,
    };
    let mut rfds: libc::fd_set = unsafe { std::mem::zeroed() };
    unsafe { libc::FD_SET(tty_fd, &mut rfds) };
    let ready = unsafe {
        libc::select(
            tty_fd + 1,
            &mut rfds,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut tv,
        )
    };
    if ready <= 0 {
        return None;
    }

    let mut buf = [0u8; 32];
    let n = tty.read(&mut buf).ok()?;
    parse_cell_pixel_response(std::str::from_utf8(&buf[..n]).ok()?)
}

/// Parse a `CSI 6 ; <cell_height_px> ; <cell_width_px> t` response.
/// Returns `(cell_width_px, cell_height_px)`.
fn parse_cell_pixel_response(s: &str) -> Option<(u32, u32)> {
    let inner = s.strip_prefix("\x1b[6;")?.strip_suffix('t')?;
    let (h, w) = inner.split_once(';')?;
    let cell_h: u32 = h.parse().ok()?;
    let cell_w: u32 = w.parse().ok()?;
    (cell_w > 0 && cell_h > 0).then_some((cell_w, cell_h))
}

fn fallback_window_size_pixels(cells_width: u16, cells_height: u16) -> (u32, u32) {
    (
        u32::from(cells_width.max(1)) * 8,
        u32::from(cells_height.max(1)) * 16,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cell_pixel_response_reads_cell_dimensions() {
        assert_eq!(parse_cell_pixel_response("\x1b[6;20;10t"), Some((10, 20)));
    }

    #[test]
    fn parse_cell_pixel_response_rejects_zero_dimensions() {
        assert_eq!(parse_cell_pixel_response("\x1b[6;0;10t"), None);
        assert_eq!(parse_cell_pixel_response("\x1b[6;20;0t"), None);
    }

    #[test]
    fn parse_cell_pixel_response_rejects_malformed_input() {
        assert_eq!(parse_cell_pixel_response("not a response"), None);
        assert_eq!(parse_cell_pixel_response("\x1b[6;20t"), None);
    }

    #[test]
    fn fallback_window_size_pixels_uses_reasonable_cell_defaults() {
        assert_eq!(fallback_window_size_pixels(100, 40), (800, 640));
        assert_eq!(fallback_window_size_pixels(0, 0), (8, 16));
    }
}
