//! The hand-written Rust equivalent of `codecs.ignite`'s executable: the
//! same 16 rounds of SHA-1 and base64 over the same 1 MiB, the same two
//! lines printed. `bitwise::codecs_within_3x_of_hand_written_rust` builds
//! it with `rustc -O` and times both (`docs/slice.md` 27.5).

fn sha1(message: &[u8]) -> [u32; 5] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut data = message.to_vec();
    let bits = (message.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bits.to_be_bytes());
    let mut w = [0u32; 80];
    for block in data.chunks_exact(64) {
        for t in 0..16 {
            w[t] = u32::from_be_bytes([
                block[t * 4],
                block[t * 4 + 1],
                block[t * 4 + 2],
                block[t * 4 + 3],
            ]);
        }
        for t in 16..80 {
            w[t] = (w[t - 3] ^ w[t - 8] ^ w[t - 14] ^ w[t - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (t, word) in w.iter().enumerate() {
            let (f, k) = match t {
                0..20 => ((b & c) | (!b & d), 0x5A827999),
                20..40 => (b ^ c ^ d, 0x6ED9EBA1),
                40..60 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        for (slot, value) in h.iter_mut().zip([a, b, c, d, e]) {
            *slot = slot.wrapping_add(value);
        }
    }
    h
}

fn hex(words: &[u32; 5]) -> String {
    words.iter().map(|w| format!("{w:08x}")).collect()
}

fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(data.len().div_ceil(3) * 4);
    let mut chunks = data.chunks_exact(3);
    for chunk in &mut chunks {
        let v = (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]);
        out.push(ALPHABET[(v >> 18) as usize]);
        out.push(ALPHABET[(v >> 12 & 63) as usize]);
        out.push(ALPHABET[(v >> 6 & 63) as usize]);
        out.push(ALPHABET[(v & 63) as usize]);
    }
    match chunks.remainder() {
        [a] => {
            let v = u32::from(*a) << 16;
            out.push(ALPHABET[(v >> 18) as usize]);
            out.push(ALPHABET[(v >> 12 & 63) as usize]);
            out.extend_from_slice(b"==");
        }
        [a, b] => {
            let v = (u32::from(*a) << 16) | (u32::from(*b) << 8);
            out.push(ALPHABET[(v >> 18) as usize]);
            out.push(ALPHABET[(v >> 12 & 63) as usize]);
            out.push(ALPHABET[(v >> 6 & 63) as usize]);
            out.push(b'=');
        }
        _ => {}
    }
    String::from_utf8(out).expect("base64 is ASCII")
}

fn main() {
    let mut data: Vec<u8> = (0..1_048_576u64).map(|i| (i * 31 + 7) as u8).collect();
    let mut digest = String::new();
    let mut encoded = 0;
    for round in 0..16u8 {
        data[0] = round;
        digest = hex(&sha1(&data));
        encoded += base64(&data).len();
    }
    println!("{digest}");
    println!("{encoded}");
}
