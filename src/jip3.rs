//! JIP-3 telemetry: the messages a node sends to `--telemetry HOST:PORT`, read into pane lines
//! (https://github.com/polkadot-fellows/JIPs/blob/main/JIP-3.md)
use std::{io::Read, net::TcpListener};

/// Accept the node's connections on `listener`, one at a time (a restarted node connects
/// again), and pass the line of each event `line` knows to `on_line`
pub fn serve(listener: TcpListener, mut on_line: impl FnMut(&str)) {
    for mut stream in listener.incoming().flatten() {
        // The first message is the node information
        let messages = std::iter::from_fn(|| read_message(&mut stream)).skip(1);
        for text in messages.filter_map(|m| line(&m)) {
            on_line(&text);
        }
    }
}

/// One JIP-3 message: its size as a little-endian u32, then the content
fn read_message(stream: &mut impl Read) -> Option<Vec<u8>> {
    let mut size = [0; 4];
    stream.read_exact(&mut size).ok()?;
    let size = u32::from_le_bytes(size) as usize;
    if size > 1 << 24 {
        return None;
    }
    let mut message = vec![0; size];
    stream.read_exact(&mut message).ok()?;
    Some(message)
}

/// The pane line for a JIP-3 event, which starts with a timestamp (u64) and a discriminator:
/// best and finalized block (slot u32, header hash), sync status (bool), failed authoring,
/// verification or execution (event ID u64, reason) and a misbehaving peer (peer ID, reason).
/// None for the other events
fn line(message: &[u8]) -> Option<String> {
    let (&kind, body) = message.get(8..)?.split_first()?;
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let block = |what: &str| {
        let slot = u32::from_le_bytes(body.get(..4)?.try_into().ok()?);
        Some(format!("{what} {slot} 0x{}", hex(body.get(4..36)?)))
    };
    let line = match kind {
        11 => block("best block")?,
        12 => block("finalized")?,
        13 => format!("in sync: {}", if *body.first()? == 1 { "yes" } else { "no" }),
        41 => format!("authoring failed: {}", reason(body.get(8..)?)?),
        44 => format!("block verification failed: {}", reason(body.get(8..)?)?),
        46 => format!("block execution failed: {}", reason(body.get(8..)?)?),
        28 => format!("peer misbehaved: 0x{}: {}", hex(body.get(..4)?), reason(body.get(32..)?)?),
        _ => return None,
    };
    Some(format!("[jip3] {line}\n"))
}

/// A JIP-3 reason: up to 128 bytes of UTF-8 after their length as a JAM variable-length natural
/// (one or two bytes), on one line
fn reason(bytes: &[u8]) -> Option<String> {
    let (&first, rest) = bytes.split_first()?;
    let extra = first.leading_ones() as usize;
    if extra > 1 {
        return None;
    }
    let high = (first as usize & (0x7f >> extra)) << (8 * extra);
    let len = high | rest.get(..extra)?.first().map_or(0, |&b| b as usize);
    let text = String::from_utf8_lossy(rest.get(extra..extra + len)?);
    Some(text.replace(['\n', '\r'], " "))
}

#[cfg(test)]
pub fn frame(content: &[u8]) -> Vec<u8> {
    [&(content.len() as u32).to_le_bytes()[..], content].concat()
}

#[cfg(test)]
pub fn best_block_event(slot: u32, hash: u8) -> Vec<u8> {
    [&7u64.to_le_bytes()[..], &[11], &slot.to_le_bytes(), &[hash; 32]].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: u8, body: &[&[u8]]) -> Vec<u8> {
        [&7u64.to_le_bytes()[..], &[kind], &body.concat()].concat()
    }

    #[test]
    fn line_reads_block_sync_and_failure_events() {
        let line = |message: Vec<u8>| line(&message);
        let hash = "ab".repeat(32);
        let slot = 260u32.to_le_bytes();
        let id = 3u64.to_le_bytes();
        assert_eq!(line(best_block_event(260, 0xab)), Some(format!("[jip3] best block 260 0x{hash}\n")));
        let finalized = line(event(12, &[&slot, &[0xab; 32]]));
        assert_eq!(finalized, Some(format!("[jip3] finalized 260 0x{hash}\n")));
        assert_eq!(line(event(13, &[&[0]])).as_deref(), Some("[jip3] in sync: no\n"));
        assert_eq!(line(event(13, &[&[1]])).as_deref(), Some("[jip3] in sync: yes\n"));
        let failed = line(event(44, &[&id, &[8], b"bad\nroot"]));
        assert_eq!(failed.as_deref(), Some("[jip3] block verification failed: bad root\n"));
        let failed = line(event(41, &[&id, &[0]]));
        assert_eq!(failed.as_deref(), Some("[jip3] authoring failed: \n"));
        let long = "x".repeat(128);
        let failed = line(event(46, &[&id, &[0x80, 128], long.as_bytes()]));
        assert_eq!(failed, Some(format!("[jip3] block execution failed: {long}\n")));
        let peer = [&[0x1a, 0x2b, 0x3c, 0x4d][..], &[0; 28]].concat();
        let misbehaved = line(event(28, &[&peer, &[3], b"sig"]));
        assert_eq!(misbehaved.as_deref(), Some("[jip3] peer misbehaved: 0x1a2b3c4d: sig\n"));
        assert_eq!(line(event(10, &[&[0; 40]])), None);
        assert_eq!(line(event(44, &[&id, &[5], b"cut"])), None);
        assert_eq!(line(vec![0; 9]), None);
    }

    #[test]
    fn read_message_splits_frames_and_stops_at_a_cut_one() {
        let bytes = [frame(b"info"), frame(b"event"), frame(b"cut")[..5].to_vec()].concat();
        let mut stream = &bytes[..];
        assert_eq!(read_message(&mut stream).as_deref(), Some(&b"info"[..]));
        assert_eq!(read_message(&mut stream).as_deref(), Some(&b"event"[..]));
        assert_eq!(read_message(&mut stream), None);
    }
}
