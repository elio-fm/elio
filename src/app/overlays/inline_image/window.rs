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
/// The per-cell result is cached in a `OnceLock` so the terminal is only
/// queried once per process — subsequent resize events reuse the cached cell
/// size with the updated cell counts.
#[cfg(unix)]
fn query_pixels_from_cell_size(cells_width: u16, cells_height: u16) -> Option<(u32, u32)> {
    static CELL_PX: OnceLock<Option<(u32, u32)>> = OnceLock::new();
    let (cell_w, cell_h) = (*CELL_PX.get_or_init(query_cell_pixel_size_from_terminal))?;
    Some((
        cell_w * u32::from(cells_width.max(1)),
        cell_h * u32::from(cells_height.max(1)),
    ))
}

#[cfg(windows)]
fn query_pixels_from_cell_size(cells_width: u16, cells_height: u16) -> Option<(u32, u32)> {
    static CELL_PX: OnceLock<Option<(u32, u32)>> = OnceLock::new();
    let (cell_w, cell_h) = (*CELL_PX.get_or_init(query_cell_pixel_size_from_terminal))?;
    Some((
        cell_w * u32::from(cells_width.max(1)),
        cell_h * u32::from(cells_height.max(1)),
    ))
}

#[cfg(not(any(unix, windows)))]
fn query_pixels_from_cell_size(_cells_width: u16, _cells_height: u16) -> Option<(u32, u32)> {
    None
}

/// Unix: send `CSI 16 t` to stdout, read the `CSI 6 ; <ph> ; <pw> t`
/// response from stdin (fd 0) via `select` + `read`, accumulating bytes
/// until the `t` terminator arrives or 300 ms elapse.
#[cfg(unix)]
fn query_cell_pixel_size_from_terminal() -> Option<(u32, u32)> {
    use std::io::Write;
    use std::time::{Duration, Instant};

    let mut stdout = std::io::stdout();
    stdout.write_all(b"\x1b[16t").ok()?;
    stdout.flush().ok()?;

    let stdin_fd = libc::STDIN_FILENO;
    let deadline = Instant::now() + Duration::from_millis(300);
    let mut buf = [0u8; 64];
    let mut filled = 0usize;

    loop {
        let remaining_us = deadline
            .saturating_duration_since(Instant::now())
            .as_micros()
            .min(300_000) as libc::suseconds_t;
        if remaining_us == 0 {
            return None;
        }

        let mut tv = libc::timeval {
            tv_sec: 0,
            tv_usec: remaining_us,
        };
        let mut rfds: libc::fd_set = unsafe { std::mem::zeroed() };
        unsafe { libc::FD_SET(stdin_fd, &mut rfds) };
        let ready = unsafe {
            libc::select(
                stdin_fd + 1,
                &mut rfds,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut tv,
            )
        };
        if ready <= 0 {
            return None;
        }

        let space = buf.len() - filled;
        if space == 0 {
            return None;
        }
        let n = unsafe {
            libc::read(
                stdin_fd,
                buf[filled..].as_mut_ptr() as *mut libc::c_void,
                space,
            )
        };
        if n <= 0 {
            return None;
        }
        filled += n as usize;

        if buf[..filled].contains(&b't') {
            break;
        }
    }

    parse_cell_pixel_response(std::str::from_utf8(&buf[..filled]).ok()?)
}

/// Windows: send `CSI 16 t` to stdout, then read the response from the
/// console stdin handle using `WaitForSingleObject` + `ReadFile` with a
/// 300 ms deadline.  Crossterm enables `ENABLE_VIRTUAL_TERMINAL_INPUT` as
/// part of raw mode, so terminal responses arrive as raw bytes via `ReadFile`.
#[cfg(windows)]
fn query_cell_pixel_size_from_terminal() -> Option<(u32, u32)> {
    use std::ffi::c_void;
    use std::io::Write;
    use std::time::{Duration, Instant};

    // STD_INPUT_HANDLE  = (DWORD)(-10)
    const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6_u32;
    const WAIT_OBJECT_0: u32 = 0x00000000;
    const INVALID_HANDLE_VALUE: *mut c_void = usize::MAX as *mut c_void;

    unsafe extern "system" {
        fn GetStdHandle(nStdHandle: u32) -> *mut c_void;
        fn WaitForSingleObject(hHandle: *mut c_void, dwMilliseconds: u32) -> u32;
        fn ReadFile(
            hFile: *mut c_void,
            lpBuffer: *mut u8,
            nNumberOfBytesToRead: u32,
            lpNumberOfBytesRead: *mut u32,
            lpOverlapped: *mut c_void,
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

/// Search `s` for the `CSI 6 ; <cell_height_px> ; <cell_width_px> t`
/// response pattern and return `(cell_width_px, cell_height_px)`.
///
/// Searches rather than anchoring at position 0 so that any key-event bytes
/// that arrived before the terminal's reply don't cause a parse failure.
fn parse_cell_pixel_response(s: &str) -> Option<(u32, u32)> {
    let start = s.find("\x1b[6;")?;
    let rest = &s[start + 4..]; // skip ESC [ 6 ;
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
        // Buffered key events before the escape sequence should not block parsing.
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
}
