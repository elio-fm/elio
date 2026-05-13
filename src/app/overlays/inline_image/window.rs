use super::TerminalWindowSize;
use crossterm::terminal;
use std::env;
#[cfg(windows)]
use std::sync::OnceLock;

// Reject TIOCGWINSZ pixels outside these per-cell bounds: tmux on WSL/ConPTY
// leaks Windows Terminal's full-window pixel size as if it were the pane's.
const MIN_PLAUSIBLE_CELL_PX_WIDTH: u32 = 4;
const MAX_PLAUSIBLE_CELL_PX_WIDTH: u32 = 20;
const MIN_PLAUSIBLE_CELL_PX_HEIGHT: u32 = 8;
const MAX_PLAUSIBLE_CELL_PX_HEIGHT: u32 = 40;

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
            if width == 0 || height == 0 {
                return None;
            }
            if env::var_os("TMUX").is_some()
                && !is_plausible_cell_pixel_ratio(width, height, cells_width, cells_height)
            {
                return None;
            }
            Some((width, height))
        })
        .or_else(|| query_windows_terminal_pixels_from_cell_size(cells_width, cells_height))
        .unwrap_or_else(|| fallback_window_size_pixels(cells_width, cells_height));
    Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width,
        pixels_height,
    })
}

fn is_plausible_cell_pixel_ratio(
    pixels_width: u32,
    pixels_height: u32,
    cells_width: u16,
    cells_height: u16,
) -> bool {
    let cell_px_w = pixels_width / u32::from(cells_width.max(1));
    let cell_px_h = pixels_height / u32::from(cells_height.max(1));
    (MIN_PLAUSIBLE_CELL_PX_WIDTH..=MAX_PLAUSIBLE_CELL_PX_WIDTH).contains(&cell_px_w)
        && (MIN_PLAUSIBLE_CELL_PX_HEIGHT..=MAX_PLAUSIBLE_CELL_PX_HEIGHT).contains(&cell_px_h)
}

#[cfg(windows)]
fn query_windows_terminal_pixels_from_cell_size(
    cells_width: u16,
    cells_height: u16,
) -> Option<(u32, u32)> {
    static CELL_PX: OnceLock<Option<(u32, u32)>> = OnceLock::new();

    if env::var_os("WT_SESSION").is_none() {
        return None;
    }

    let (cell_w, cell_h) = (*CELL_PX.get_or_init(query_windows_terminal_cell_pixel_size))?;
    Some((
        cell_w * u32::from(cells_width.max(1)),
        cell_h * u32::from(cells_height.max(1)),
    ))
}

#[cfg(not(windows))]
fn query_windows_terminal_pixels_from_cell_size(
    _cells_width: u16,
    _cells_height: u16,
) -> Option<(u32, u32)> {
    None
}

#[cfg(windows)]
fn query_windows_terminal_cell_pixel_size() -> Option<(u32, u32)> {
    use std::ffi::c_void;
    use std::io::Write;
    use std::time::{Duration, Instant};

    const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6;
    const WAIT_OBJECT_0: u32 = 0x0000_0000;
    const INVALID_HANDLE_VALUE: *mut c_void = usize::MAX as *mut c_void;

    unsafe extern "system" {
        fn GetStdHandle(n_std_handle: u32) -> *mut c_void;
        fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
        fn ReadFile(
            file: *mut c_void,
            buffer: *mut u8,
            bytes_to_read: u32,
            bytes_read: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
    }

    let mut stdout = std::io::stdout();
    stdout.write_all(b"\x1b[16t").ok()?;
    stdout.flush().ok()?;

    let stdin_handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if stdin_handle.is_null() || stdin_handle == INVALID_HANDLE_VALUE {
        return None;
    }

    let deadline = Instant::now() + Duration::from_millis(300);
    let mut buf = [0u8; 64];
    let mut filled = 0usize;

    loop {
        let remaining_ms = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(300) as u32;
        if remaining_ms == 0 {
            return None;
        }

        let wait_result = unsafe { WaitForSingleObject(stdin_handle, remaining_ms) };
        if wait_result != WAIT_OBJECT_0 {
            return None;
        }

        let space = (buf.len() - filled) as u32;
        if space == 0 {
            return None;
        }
        let mut bytes_read = 0u32;
        let ok = unsafe {
            ReadFile(
                stdin_handle,
                buf[filled..].as_mut_ptr(),
                space,
                &mut bytes_read,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 || bytes_read == 0 {
            return None;
        }
        filled += bytes_read as usize;

        if buf[..filled].contains(&b't') {
            break;
        }
    }

    parse_cell_pixel_response(std::str::from_utf8(&buf[..filled]).ok()?)
}

#[cfg(any(test, windows))]
fn parse_cell_pixel_response(s: &str) -> Option<(u32, u32)> {
    let start = s.find("\x1b[6;")?;
    let rest = &s[start + 4..];
    let end = rest.find('t')?;
    let (h, w) = rest[..end].split_once(';')?;
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
    fn parse_cell_pixel_response_finds_response_after_leading_bytes() {
        assert_eq!(parse_cell_pixel_response("ab\x1b[6;20;10t"), Some((10, 20)));
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

    #[test]
    fn plausible_cell_pixel_ratio_filters_leaked_window_pixels() {
        assert!(is_plausible_cell_pixel_ratio(800, 480, 80, 24)); // 10×20
        assert!(is_plausible_cell_pixel_ratio(1280, 768, 80, 24)); // 16×32
        assert!(!is_plausible_cell_pixel_ratio(1920, 1080, 80, 24)); // 24×45
        assert!(!is_plausible_cell_pixel_ratio(10, 10, 80, 24)); // 0×0
    }
}
