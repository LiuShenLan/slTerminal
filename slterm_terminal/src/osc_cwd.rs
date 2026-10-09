//! Sniffer for OSC sequences the vte parser drops: OSC 7 / OSC 9;9 (working
//! directory) and OSC 133;A (semantic prompt mark).
//!
//! The vte parser Slterm uses (crates.io `vte` 0.15) does not decode OSC 7
//! (`file://` URI), OSC 9;9 (working directory) or OSC 133 (semantic prompt zones) —
//! it logs them as "unhandled" and drops them. Rather than fork the parser, we
//! tee the raw PTY byte stream through this tiny state machine.
//!
//! Each recognized event is returned tagged with the byte offset **just past
//! its terminator** within the fed chunk. Prompt marks need the grid cursor
//! exactly where the shell emitted the sequence, so the PTY reader splits its
//! `parser.advance` call at these offsets and applies each mark in between —
//! zero vte changes, perfect cursor accuracy.
//!
//! On Windows, Nushell defaults to OSC 9;9 for working-directory reports
//! (its OSC 7 reporting is off by default there),
//! while PowerShell/pwsh and most Unix shells use OSC 7. We accept both.
//! OSC 133;A comes from Slterm's own shell integration (PS1/prompt hooks) or
//! natively from shells like Nushell.
//!
//! The state machine survives an OSC split across read chunks, and stops
//! accumulating as soon as a payload can't be one of ours — so an unrelated
//! but huge OSC (e.g. an OSC 52 clipboard blob) never grows our buffer.

/// Cap on a single OSC payload we're willing to buffer. A real cwd path is far
/// shorter; anything longer is not a directory report and gets dropped.
const MAX_PAYLOAD: usize = 4096;

/// Cap for OSC 1337 inline-image payloads (metadata plus base64). Anything
/// larger is dropped while it is still arriving rather than buffered forever.
const MAX_IMAGE_PAYLOAD: usize = 12 * 1024 * 1024;
/// A compressed image can expand by orders of magnitude. Keep a 4K screenshot
/// comfortably inside the budget, but reject image bombs before a decoder sees
/// them. The frontend repeats this check as a defense-in-depth boundary.
const MAX_IMAGE_PIXELS: u64 = 16 * 1024 * 1024;

/// An OSC event recognized by the sniffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscEvent {
    /// OSC 7 / 9;9 — the shell reported its working directory (native path).
    Cwd(String),
    /// OSC 133;A — the shell is about to draw a prompt (semantic zone start).
    PromptMark,
    /// OSC 133;B — the prompt ended; subsequent cells contain shell input.
    PromptInput,
    /// OSC 133;C — a command started executing.
    CommandStart,
    /// OSC 133;D — the command finished. `exit_code` is the first parameter
    /// (`133;D;<code>[;aid=…]`), reported by Slterm's own shell integration;
    /// third-party integrations that send a bare `133;D` yield `None`.
    CommandDone { exit_code: Option<i32> },
    /// OSC 1337 `SetUserVar=<name>=<b64>` — a shell-integration variable
    /// (the OSC 1337 shell-integration convention).
    UserVar { name: String, value: String },
    /// OSC 9 — free-text program notification.
    Notify(String),
    /// OSC 9;4 — ConEmu 任务进度。`state` 是原始状态码，`value` 是 0..=100 的
    /// 百分比（只有 state 1 和 4 带值）。
    ///
    /// ConEmu 只定义 0 清除 / 1 正常 / 2 错误 / 3 不确定 / 4 暂停。这里不在解析
    /// 层做语义收窄：部分 shell 集成实测会用规范外的 `9;4;5;0` 表示「成功完成」，
    /// 把未知码当成非法而丢掉，等于让进度条卡在最后一个状态上
    /// 永远不消失。映射交给消费端。
    Progress { state: u8, value: Option<u8> },
    /// OSC 1337 `File=...inline=1:<base64>` — an inline image.
    /// Only static PNG/JPEG/GIF input is accepted; animated GIFs are rendered
    /// as their first frame by the frontend.
    /// `width`/`height` come from the encoded image header, in pixels.
    InlineImage {
        data: Vec<u8>,
        width: u32,
        height: u32,
    },
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Outside any escape sequence.
    #[default]
    Ground,
    /// Saw `ESC` (0x1b).
    Esc,
    /// Inside `ESC ]` … collecting the OSC payload.
    Osc,
    /// Inside an OSC and saw `ESC` — maybe the `ESC \` string terminator.
    OscEsc,
}

/// Streaming OSC 7 / 9;9 / 133;A sniffer. Feed it every PTY byte; it returns
/// the recognized events, each tagged with the offset just past its
/// terminator (the `parser.advance` split point).
#[derive(Default)]
pub struct CwdSniffer {
    phase: Phase,
    payload: Vec<u8>,
    /// Cleared once the payload's prefix rules out all sequences we care
    /// about, so the rest of that (irrelevant) OSC is skipped unbuffered.
    interested: bool,
}

impl CwdSniffer {
    /// Feed a chunk of raw PTY output. Returns all complete events within the
    /// chunk in order, tagged with the byte offset just past each terminator.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<(usize, OscEvent)> {
        let mut events = Vec::new();
        for (i, &b) in bytes.iter().enumerate() {
            match self.phase {
                Phase::Ground => {
                    if b == 0x1b {
                        self.phase = Phase::Esc;
                    }
                }
                Phase::Esc => match b {
                    b']' => {
                        self.phase = Phase::Osc;
                        self.payload.clear();
                        self.interested = true;
                    }
                    0x1b => {} // another ESC: stay armed
                    _ => self.phase = Phase::Ground,
                },
                Phase::Osc => self.step_osc(b, i, &mut events),
                Phase::OscEsc => {
                    if b == b'\\' {
                        // ST terminator (ESC \).
                        if let Some(event) = self.parse() {
                            events.push((i + 1, event));
                        }
                        self.reset_to_ground();
                    } else {
                        // The ESC belonged to the payload after all; keep it and
                        // reprocess this byte in the normal OSC state.
                        self.push(0x1b);
                        self.phase = Phase::Osc;
                        self.step_osc(b, i, &mut events);
                    }
                }
            }
        }
        events
    }

    /// Handle one byte while inside an OSC payload. `i` is the byte's offset
    /// in the fed chunk, used to tag completed events.
    fn step_osc(&mut self, b: u8, i: usize, events: &mut Vec<(usize, OscEvent)>) {
        match b {
            0x07 => {
                // BEL terminator.
                if let Some(event) = self.parse() {
                    events.push((i + 1, event));
                }
                self.reset_to_ground();
            }
            0x1b => self.phase = Phase::OscEsc,
            _ => self.push(b),
        }
    }

    fn reset_to_ground(&mut self) {
        self.phase = Phase::Ground;
        self.payload.clear();
        self.interested = false;
    }

    /// Append a payload byte, giving up early once the prefix can't match.
    fn push(&mut self, b: u8) {
        if !self.interested {
            return;
        }
        // Inline images are the one legitimately huge OSC we buffer.
        let cap = if self.payload.starts_with(b"1337;") {
            MAX_IMAGE_PAYLOAD
        } else {
            MAX_PAYLOAD
        };
        if self.payload.len() >= cap {
            self.interested = false;
            return;
        }
        self.payload.push(b);
        // Decide as soon as we have enough bytes to compare against the
        // prefixes we care about ("7;", "9;9;", "133;" and "1337;").
        if self.payload.len() <= 4 && !prefix_could_match(&self.payload) {
            self.interested = false;
        }
    }

    /// Parse a completed payload into an event.
    fn parse(&self) -> Option<OscEvent> {
        if let Some(rest) = self.payload.strip_prefix(b"7;") {
            return parse_osc7_uri(rest).map(OscEvent::Cwd);
        }
        if let Some(rest) = self.payload.strip_prefix(b"9;9;") {
            let s = String::from_utf8_lossy(rest);
            let s = s.trim().trim_end_matches(['/', '\\']);
            return (!s.is_empty()).then(|| OscEvent::Cwd(s.to_string()));
        }
        if let Some(rest) = self.payload.strip_prefix(b"1337;") {
            if let Some(var) = rest.strip_prefix(b"SetUserVar=") {
                return parse_user_var(var);
            }
            return parse_osc1337_image(rest);
        }
        if let Some(rest) = self.payload.strip_prefix(b"133;") {
            // Semantic prompt zones. `A` may carry optional
            // `;key=value` params — accept those too.
            let phased = |ch: u8| rest.first() == Some(&ch) && (rest.len() == 1 || rest[1] == b';');
            if phased(b'A') {
                return Some(OscEvent::PromptMark);
            }
            if phased(b'B') {
                return Some(OscEvent::PromptInput);
            }
            if phased(b'C') {
                return Some(OscEvent::CommandStart);
            }
            if phased(b'D') {
                // `D;<code>[;aid=…]` — take the first parameter when it is a
                // plain integer; a bare `D` or junk parameter reports None.
                let exit_code = rest
                    .get(2..)
                    .map(|params| params.split(|&b| b == b';').next().unwrap_or(params))
                    .and_then(|first| std::str::from_utf8(first).ok())
                    .and_then(|first| first.trim().parse::<i32>().ok());
                return Some(OscEvent::CommandDone { exit_code });
            }
            return None;
        }
        if let Some(rest) = self.payload.strip_prefix(b"9;") {
            // OSC 9 family. `9;9;` (cwd) matched above; `9;4;` reports
            // progress; anything else is a free-text notification.
            if let Some(progress) = rest.strip_prefix(b"4;") {
                let mut fields = progress.split(|&b| b == b';');
                let state = fields
                    .next()
                    .and_then(|field| std::str::from_utf8(field).ok())
                    .and_then(|field| field.trim().parse::<u8>().ok())?;
                let value = fields
                    .next()
                    .and_then(|field| std::str::from_utf8(field).ok())
                    .and_then(|field| field.trim().parse::<u8>().ok())
                    .map(|percent| percent.min(100));
                return Some(OscEvent::Progress { state, value });
            }
            let text = String::from_utf8_lossy(rest).trim().to_owned();
            return (!text.is_empty()).then_some(OscEvent::Notify(text));
        }
        None
    }
}

/// Whether `payload` is a prefix of, or prefixed by, one of our OSC numbers.
fn prefix_could_match(payload: &[u8]) -> bool {
    const A: &[u8] = b"7;";
    const B: &[u8] = b"9;";
    const C: &[u8] = b"133;";
    const D: &[u8] = b"1337;";
    let matches = |target: &[u8]| target.starts_with(payload) || payload.starts_with(target);
    matches(A) || matches(B) || matches(C) || matches(D)
}

/// Parse a `SetUserVar=<name>=<base64>` body. Names are restricted to the
/// word-character set every emitter in the wild uses; the value must decode
/// to UTF-8 (these are shell-integration strings, not blobs). A var larger
/// than 8 KiB is not a query — reject rather than ferry it around.
fn parse_user_var(rest: &[u8]) -> Option<OscEvent> {
    use base64::Engine as _;

    let eq = rest.iter().position(|&b| b == b'=')?;
    let name = std::str::from_utf8(&rest[..eq]).ok()?;
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return None;
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&rest[eq + 1..])
        .ok()?;
    if decoded.len() > 8 * 1024 {
        return None;
    }
    let value = String::from_utf8(decoded).ok()?;
    Some(OscEvent::UserVar {
        name: name.to_owned(),
        value,
    })
}

/// Parse an OSC 1337 body (`File=key=value;...:<base64>`) into an inline
/// image event. Only `inline=1` PNG/JPEG/GIF payloads are accepted;
/// `width`/`height` come from the encoded file rather than terminal params, so
/// broken or hostile metadata cannot lie about the allocation size.
fn parse_osc1337_image(rest: &[u8]) -> Option<OscEvent> {
    use base64::Engine as _;

    let rest = rest.strip_prefix(b"File=")?;
    let colon = rest.iter().position(|&b| b == b':')?;
    let (args, data) = (&rest[..colon], &rest[colon + 1..]);

    // `inline=1` is required — without it the protocol semantics are "download".
    let inline = args.split(|&b| b == b';').any(|arg| arg == b"inline=1");
    if !inline {
        return None;
    }

    let data = base64::engine::general_purpose::STANDARD
        .decode(data)
        .or_else(|_| {
            // Some emitters wrap base64 in whitespace/newlines; strip and retry.
            let cleaned: Vec<u8> = data
                .iter()
                .copied()
                .filter(|b| !b.is_ascii_whitespace())
                .collect();
            base64::engine::general_purpose::STANDARD.decode(&cleaned)
        })
        .ok()?;

    let (width, height) = image_dimensions(&data)?;
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    if pixels > MAX_IMAGE_PIXELS {
        return None;
    }
    Some(OscEvent::InlineImage {
        data,
        width,
        height,
    })
}

/// Read dimensions from the supported formats without allocating their pixel
/// buffers. This is deliberately a small sniffer, not a decoder: the frontend
/// still validates and decodes the bytes in a bounded background job.
fn image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    png_dimensions(data)
        .or_else(|| gif_dimensions(data))
        .or_else(|| jpeg_dimensions(data))
}

fn png_dimensions(png: &[u8]) -> Option<(u32, u32)> {
    const MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
    if png.len() < 24 || !png.starts_with(MAGIC) || &png[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(png[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(png[20..24].try_into().ok()?);
    valid_dimensions(width, height)
}

fn gif_dimensions(gif: &[u8]) -> Option<(u32, u32)> {
    if gif.len() < 10 || !(gif.starts_with(b"GIF87a") || gif.starts_with(b"GIF89a")) {
        return None;
    }
    let width = u16::from_le_bytes(gif[6..8].try_into().ok()?) as u32;
    let height = u16::from_le_bytes(gif[8..10].try_into().ok()?) as u32;
    valid_dimensions(width, height)
}

fn jpeg_dimensions(jpeg: &[u8]) -> Option<(u32, u32)> {
    if jpeg.len() < 4 || !jpeg.starts_with(&[0xff, 0xd8]) {
        return None;
    }

    let mut offset = 2;
    while offset < jpeg.len() {
        while jpeg.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let marker = *jpeg.get(offset)?;
        offset += 1;

        // Standalone markers have no length field.
        if marker == 0xd8 || marker == 0xd9 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }

        let segment_len =
            u16::from_be_bytes(jpeg.get(offset..offset + 2)?.try_into().ok()?) as usize;
        if segment_len < 2 || offset.checked_add(segment_len)? > jpeg.len() {
            return None;
        }

        let is_start_of_frame = matches!(
            marker,
            0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf
        );
        if is_start_of_frame {
            if segment_len < 7 {
                return None;
            }
            let height =
                u16::from_be_bytes(jpeg.get(offset + 3..offset + 5)?.try_into().ok()?) as u32;
            let width =
                u16::from_be_bytes(jpeg.get(offset + 5..offset + 7)?.try_into().ok()?) as u32;
            return valid_dimensions(width, height);
        }

        // Start-of-scan is followed by entropy-coded data; a valid dimensions
        // marker must have appeared before it.
        if marker == 0xda {
            return None;
        }
        offset += segment_len;
    }
    None
}

fn valid_dimensions(width: u32, height: u32) -> Option<(u32, u32)> {
    (width > 0 && height > 0 && u64::from(width) * u64::from(height) <= MAX_IMAGE_PIXELS)
        .then_some((width, height))
}

/// Decode an OSC 7 body (`file://HOST/PATH`) into a native path.
fn parse_osc7_uri(rest: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(rest).ok()?.trim();
    let after = s.strip_prefix("file://").unwrap_or(s);

    // `after` is `HOST/PATH`; the path starts at the first '/'. An empty host
    // (`file:///C:/…`) leaves the slash at index 0.
    let slash = after.find('/')?;
    let decoded = percent_decode(&after[slash..]);

    // Windows drive paths arrive as "/C:/Users/…"; strip the leading slash.
    let cleaned = if is_windows_drive_path(&decoded) {
        decoded[1..].to_string()
    } else {
        decoded
    };

    (!cleaned.is_empty()).then_some(cleaned)
}

/// True for "/C:/…" style paths that need their leading slash removed.
fn is_windows_drive_path(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':'
}

/// Minimal percent-decoding (`%20` → space, etc.); leaves malformed escapes as-is.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex_val(b[i + 1]), hex_val(b[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}
