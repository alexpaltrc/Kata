//! DEFLATE compression (RFC 1951) inside a zlib stream (RFC 1950): how PNG stores
//! its pixels.
//!
//! Repeats are found through hash chains, and each one is settled a byte late, in
//! case a longer one starts there. Every block then gets Huffman codes of its own,
//! the shortest that fit DEFLATE's limits, found by package-merge.

/// How far back a repeat can reach.
const WINDOW: usize = 1 << 15;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
/// Earlier positions a search tries before settling for the best so far.
const MAX_CHAIN: usize = 64;
/// A repeat this long ends the search.
const NICE_MATCH: usize = 128;
/// A repeat this long is taken at once, without trying the next byte.
const LAZY_ENOUGH: usize = 32;
/// While a repeat this long is held, the next byte gets a shorter search.
const GOOD_MATCH: usize = 8;
/// Positions are chained by their first four bytes: chaining by three finds more
/// short repeats, which cost about as much as the bytes they replace.
const HASHED: usize = 4;
const HASH_BITS: u32 = 15;
/// Literals and repeats gathered before a block is written.
const BLOCK_TOKENS: usize = 1 << 15;
/// No earlier position.
const NONE: u32 = u32::MAX;

const END_OF_BLOCK: usize = 256;
/// The order in which a block lists its code-length code.
const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// `data` compressed into a zlib stream.
pub fn zlib(data: &[u8]) -> Vec<u8> {
    let mut bits = Bits::default();
    // DEFLATE with a 32 KiB window; the second byte makes the pair a multiple of 31.
    bits.out.extend_from_slice(&[0x78, 0x9C]);
    deflate(data, &mut bits);
    let mut out = bits.finish();
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn deflate(data: &[u8], bits: &mut Bits) {
    let mut chains = Chains::new(data);
    let mut tokens = Vec::with_capacity(BLOCK_TOKENS + 2);
    // A repeat found one byte back, held while this byte is tried for a longer one.
    let mut held: Option<(usize, usize)> = None;
    let mut pos = 0;
    while pos < data.len() {
        if tokens.len() >= BLOCK_TOKENS {
            write_block(&tokens, false, bits);
            tokens.clear();
        }
        let chain = if held.is_some_and(|(length, _)| length >= GOOD_MATCH) {
            MAX_CHAIN / 4
        } else {
            MAX_CHAIN
        };
        let found = chains.find(pos, chain);
        if let Some((length, distance)) = held.take() {
            if found.0 <= length {
                tokens.push(Token::repeat(length, distance));
                for skipped in pos + 1..pos - 1 + length {
                    chains.insert(skipped);
                }
                pos += length - 1;
                continue;
            }
            tokens.push(Token::literal(data[pos - 1]));
        }
        let (length, distance) = found;
        if length >= LAZY_ENOUGH {
            tokens.push(Token::repeat(length, distance));
            for skipped in pos + 1..pos + length {
                chains.insert(skipped);
            }
            pos += length;
        } else if length >= MIN_MATCH {
            held = Some(found);
            pos += 1;
        } else {
            tokens.push(Token::literal(data[pos]));
            pos += 1;
        }
    }
    write_block(&tokens, true, bits);
}

/// A literal byte (`distance` 0), or a repeat of `length` bytes from `distance` back.
#[derive(Clone, Copy)]
struct Token {
    length: u16,
    distance: u16,
}

impl Token {
    const fn literal(byte: u8) -> Self {
        Self {
            length: byte as u16,
            distance: 0,
        }
    }

    const fn repeat(length: usize, distance: usize) -> Self {
        Self {
            length: length as u16,
            distance: distance as u16,
        }
    }
}

/// Every earlier position, chained by the hash of the bytes starting there.
struct Chains<'a> {
    data: &'a [u8],
    /// The latest position with each hash.
    head: Vec<u32>,
    /// The position before each one with the same hash, within the window.
    prev: Vec<u32>,
}

impl<'a> Chains<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            head: vec![NONE; 1 << HASH_BITS],
            prev: vec![NONE; WINDOW],
        }
    }

    fn hash(&self, pos: usize) -> usize {
        let key = u32::from_le_bytes(self.data[pos..pos + HASHED].try_into().unwrap());
        (key.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
    }

    /// Adds `pos` for later searches.
    fn insert(&mut self, pos: usize) {
        if pos + HASHED <= self.data.len() {
            let hash = self.hash(pos);
            self.link(pos, hash);
        }
    }

    fn link(&mut self, pos: usize, hash: usize) {
        self.prev[pos % WINDOW] = self.head[hash];
        self.head[hash] = pos as u32;
    }

    /// The longest earlier repeat of what starts at `pos`, as `(length, distance)`,
    /// or a length of 0. Adds `pos` for later searches.
    fn find(&mut self, pos: usize, chain: usize) -> (usize, usize) {
        let data = self.data;
        let max = (data.len() - pos).min(MAX_MATCH);
        if max < HASHED {
            return (0, 0);
        }
        let hash = self.hash(pos);
        let nice = max.min(NICE_MATCH);
        let mut best = (0, 0);
        let mut candidate = self.head[hash];
        for _ in 0..chain {
            if candidate == NONE {
                break;
            }
            let start = candidate as usize;
            let distance = pos - start;
            if distance > WINDOW {
                break;
            }
            // Only a repeat that reaches past the best so far can beat it.
            if data[start + best.0] == data[pos + best.0] {
                let length = match_length(data, start, pos, max);
                if length > best.0 {
                    best = (length, distance);
                    if length >= nice {
                        break;
                    }
                }
            }
            candidate = self.prev[start % WINDOW];
        }
        self.link(pos, hash);
        if best.0 >= MIN_MATCH { best } else { (0, 0) }
    }
}

/// How many bytes, up to `max`, starting at `earlier` and at `pos` are the same.
fn match_length(data: &[u8], earlier: usize, pos: usize, max: usize) -> usize {
    let word = |at: usize| u64::from_le_bytes(data[at..at + 8].try_into().unwrap());
    let mut length = 0;
    while length + 8 <= max {
        let differ = word(earlier + length) ^ word(pos + length);
        if differ != 0 {
            return length + differ.trailing_zeros() as usize / 8;
        }
        length += 8;
    }
    while length < max && data[earlier + length] == data[pos + length] {
        length += 1;
    }
    length
}

/// Writes `tokens` as one block with Huffman codes made for it.
fn write_block(tokens: &[Token], last: bool, bits: &mut Bits) {
    let mut literal_counts = [0; 286];
    let mut distance_counts = [0; 30];
    literal_counts[END_OF_BLOCK] = 1;
    for token in tokens {
        if token.distance == 0 {
            literal_counts[usize::from(token.length)] += 1;
        } else {
            literal_counts[257 + length_symbol(token.length).0] += 1;
            distance_counts[distance_symbol(token.distance).0] += 1;
        }
    }
    let literals = Code::new(&literal_counts, 15);
    let distances = Code::new(&distance_counts, 15);

    // Both codes' lengths, trimmed of unused symbols at the end, are run-length
    // encoded with a code of their own.
    let literal_len = used(&literals.lengths).max(257);
    let distance_len = used(&distances.lengths).max(1);
    let lengths: Vec<u8> = literals.lengths[..literal_len]
        .iter()
        .chain(&distances.lengths[..distance_len])
        .copied()
        .collect();
    let runs = runs(&lengths);
    let mut run_counts = [0; 19];
    for &(symbol, _) in &runs {
        run_counts[usize::from(symbol)] += 1;
    }
    let run_code = Code::new(&run_counts, 7);
    let order_len = CODE_LENGTH_ORDER
        .iter()
        .rposition(|&symbol| run_code.lengths[symbol] != 0)
        .map_or(0, |last| last + 1)
        .max(4);

    bits.put(u32::from(last), 1);
    bits.put(2, 2);
    bits.put((literal_len - 257) as u32, 5);
    bits.put((distance_len - 1) as u32, 5);
    bits.put((order_len - 4) as u32, 4);
    for &symbol in &CODE_LENGTH_ORDER[..order_len] {
        bits.put(u32::from(run_code.lengths[symbol]), 3);
    }
    for (symbol, extra) in runs {
        run_code.put(bits, usize::from(symbol));
        match symbol {
            16 => bits.put(u32::from(extra), 2),
            17 => bits.put(u32::from(extra), 3),
            18 => bits.put(u32::from(extra), 7),
            _ => {}
        }
    }

    for token in tokens {
        if token.distance == 0 {
            literals.put(bits, usize::from(token.length));
        } else {
            let (symbol, extra_bits, extra) = length_symbol(token.length);
            literals.put(bits, 257 + symbol);
            bits.put(extra, extra_bits);
            let (symbol, extra_bits, extra) = distance_symbol(token.distance);
            distances.put(bits, symbol);
            bits.put(extra, extra_bits);
        }
    }
    literals.put(bits, END_OF_BLOCK);
}

/// How many symbols there are up to the last one with a code.
fn used(lengths: &[u8]) -> usize {
    lengths
        .iter()
        .rposition(|&length| length != 0)
        .map_or(0, |last| last + 1)
}

/// Code lengths as code-length symbols with their extra values: 16 repeats the
/// previous length 3–6 times, 17 and 18 give 3–10 and 11–138 zeros.
fn runs(lengths: &[u8]) -> Vec<(u8, u8)> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < lengths.len() {
        let length = lengths[i];
        let count = lengths[i..].iter().take_while(|&&l| l == length).count();
        i += count;
        let mut left = count;
        if length == 0 {
            while left >= 11 {
                let run = left.min(138);
                runs.push((18, (run - 11) as u8));
                left -= run;
            }
            if left >= 3 {
                runs.push((17, (left - 3) as u8));
                left = 0;
            }
        } else {
            runs.push((length, 0));
            left -= 1;
            while left >= 3 {
                let run = left.min(6);
                runs.push((16, (run - 3) as u8));
                left -= run;
            }
        }
        runs.extend((0..left).map(|_| (length, 0)));
    }
    runs
}

/// The length symbol for a repeat of `length` bytes, counted from 257, with its
/// extra bits: how many, and their value.
fn length_symbol(length: u16) -> (usize, u32, u32) {
    let x = u32::from(length) - MIN_MATCH as u32;
    if x < 8 {
        return (x as usize, 0, 0);
    }
    if usize::from(length) == MAX_MATCH {
        return (28, 0, 0);
    }
    // Past the first eight, each power of two splits into four symbols.
    let top = x.ilog2();
    let extra_bits = top - 2;
    let symbol = 4 * (top - 1) + (x >> extra_bits & 3);
    (symbol as usize, extra_bits, x & ((1 << extra_bits) - 1))
}

/// The distance symbol for `distance`, with its extra bits: how many, and their
/// value.
fn distance_symbol(distance: u16) -> (usize, u32, u32) {
    let x = u32::from(distance) - 1;
    if x < 4 {
        return (x as usize, 0, 0);
    }
    // Past the first four, each power of two splits into two symbols.
    let top = x.ilog2();
    let extra_bits = top - 1;
    let symbol = 2 * top + (x >> extra_bits & 1);
    (symbol as usize, extra_bits, x & ((1 << extra_bits) - 1))
}

/// A canonical Huffman code.
struct Code {
    lengths: Vec<u8>,
    /// Each symbol's code, bit-reversed: DEFLATE writes codes first bit first.
    codes: Vec<u32>,
}

impl Code {
    /// The code that writes symbols seen `counts` times in the fewest bits, none
    /// longer than `limit`.
    fn new(counts: &[u32], limit: u8) -> Self {
        let lengths = code_lengths(counts, limit);
        let mut per_length = [0u32; 16];
        for &length in &lengths {
            per_length[usize::from(length)] += 1;
        }
        per_length[0] = 0;
        let mut next = [0u32; 16];
        let mut code = 0;
        for length in 1..16 {
            code = (code + per_length[length - 1]) << 1;
            next[length] = code;
        }
        let codes = lengths
            .iter()
            .map(|&length| {
                if length == 0 {
                    return 0;
                }
                let code = next[usize::from(length)];
                next[usize::from(length)] += 1;
                code.reverse_bits() >> (32 - u32::from(length))
            })
            .collect();
        Self { lengths, codes }
    }

    fn put(&self, bits: &mut Bits, symbol: usize) {
        bits.put(self.codes[symbol], u32::from(self.lengths[symbol]));
    }
}

/// The optimal code lengths for symbols seen `counts` times, none longer than
/// `limit`, by package-merge: at each of `limit` levels, the cheapest pairs of the
/// level below join the symbols as packages, and how often a symbol takes part in
/// the cheapest `2n - 2` items is its length.
fn code_lengths(counts: &[u32], limit: u8) -> Vec<u8> {
    #[derive(Clone, Copy)]
    enum Item {
        Symbol(usize),
        Package,
    }

    let mut symbols: Vec<(u64, usize)> = counts
        .iter()
        .enumerate()
        .filter(|&(_, &count)| count > 0)
        .map(|(symbol, &count)| (u64::from(count), symbol))
        .collect();
    // A code needs two symbols, even if one is never written.
    let mut unused = (0..counts.len()).filter(|&symbol| counts[symbol] == 0);
    while symbols.len() < 2 {
        symbols.push((1, unused.next().expect("two symbols at least")));
    }
    symbols.sort_unstable();

    let leaves: Vec<(u64, Item)> = symbols
        .iter()
        .map(|&(weight, symbol)| (weight, Item::Symbol(symbol)))
        .collect();
    let mut levels = vec![leaves.clone()];
    for _ in 1..limit {
        let below = levels.last().unwrap();
        let packages = below
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[a, b]| (a.0 + b.0, Item::Package));
        let mut level = Vec::with_capacity(leaves.len() + below.len() / 2);
        let (mut singles, mut pairs) = (leaves.iter().copied().peekable(), packages.peekable());
        while let Some(next) = match (singles.peek(), pairs.peek()) {
            (Some(single), Some(pair)) if pair.0 < single.0 => pairs.next(),
            (Some(_), _) => singles.next(),
            (None, _) => pairs.next(),
        } {
            level.push(next);
        }
        levels.push(level);
    }

    let mut lengths = vec![0; counts.len()];
    let mut take = 2 * symbols.len() - 2;
    for level in levels.iter().rev() {
        let mut packages = 0;
        for &(_, item) in &level[..take] {
            match item {
                Item::Symbol(symbol) => lengths[symbol] += 1,
                Item::Package => packages += 1,
            }
        }
        take = 2 * packages;
    }
    lengths
}

/// Bits packed first bit first, as DEFLATE wants.
#[derive(Default)]
struct Bits {
    out: Vec<u8>,
    pending: u64,
    count: u32,
}

impl Bits {
    /// Appends the low `count` bits of `value`; `count` is at most 32.
    fn put(&mut self, value: u32, count: u32) {
        self.pending |= u64::from(value) << self.count;
        self.count += count;
        if self.count >= 32 {
            self.out
                .extend_from_slice(&(self.pending as u32).to_le_bytes());
            self.pending >>= 32;
            self.count -= 32;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        let bytes = self.count.div_ceil(8) as usize;
        self.out
            .extend_from_slice(&self.pending.to_le_bytes()[..bytes]);
        self.out
    }
}

/// The zlib checksum.
fn adler32(data: &[u8]) -> u32 {
    const MODULUS: u32 = 65521;
    let (mut a, mut b) = (1u32, 0u32);
    // The most bytes that can be added up before `b` could overflow.
    for chunk in data.chunks(5552) {
        for &byte in chunk {
            a += u32::from(byte);
            b += a;
        }
        a %= MODULUS;
        b %= MODULUS;
    }
    b << 16 | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adler32_matches_its_definition() {
        assert_eq!(adler32(b""), 1);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn symbols_cover_every_length_and_distance() {
        assert_eq!(length_symbol(3), (0, 0, 0));
        assert_eq!(length_symbol(10), (7, 0, 0));
        assert_eq!(length_symbol(12), (8, 1, 1));
        assert_eq!(length_symbol(257), (27, 5, 30));
        assert_eq!(length_symbol(258), (28, 0, 0));
        assert_eq!(distance_symbol(1), (0, 0, 0));
        assert_eq!(distance_symbol(5), (4, 1, 0));
        assert_eq!(distance_symbol(8), (5, 1, 1));
        assert_eq!(distance_symbol(32768), (29, 13, 8191));
    }

    /// Kraft's sum: 1 exactly when a code wastes nothing.
    fn kraft(lengths: &[u8]) -> f64 {
        lengths
            .iter()
            .filter(|&&length| length > 0)
            .map(|&length| 0.5f64.powi(i32::from(length)))
            .sum()
    }

    #[test]
    fn code_lengths_are_complete_and_within_the_limit() {
        // Fibonacci counts want a code as deep as there are symbols.
        let mut counts = vec![1u32, 1];
        while counts.len() < 30 {
            counts.push(counts[counts.len() - 1] + counts[counts.len() - 2]);
        }
        for limit in [7, 15] {
            let lengths = code_lengths(&counts, limit);
            assert!(lengths.iter().all(|&length| (1..=limit).contains(&length)));
            assert_eq!(kraft(&lengths), 1.0);
        }
        // Without a limit to bite, it's plain Huffman.
        assert_eq!(code_lengths(&[5, 1, 1, 2, 0], 15), [1, 3, 3, 2, 0]);
    }

    #[test]
    fn a_lone_symbol_still_gets_a_code() {
        assert_eq!(code_lengths(&[0, 0, 9], 15), [1, 0, 1]);
        assert_eq!(code_lengths(&[0, 0, 0], 15), [1, 1, 0]);
    }

    #[test]
    fn runs_rebuild_the_lengths() {
        let lengths: Vec<u8> = [vec![0; 150], vec![8; 9], vec![5, 5], vec![0; 4]].concat();
        let mut rebuilt = Vec::new();
        for (symbol, extra) in runs(&lengths) {
            let extra = usize::from(extra);
            match symbol {
                16 => rebuilt.extend(std::iter::repeat_n(*rebuilt.last().unwrap(), 3 + extra)),
                17 => rebuilt.extend(std::iter::repeat_n(0, 3 + extra)),
                18 => rebuilt.extend(std::iter::repeat_n(0, 11 + extra)),
                length => rebuilt.push(length),
            }
        }
        assert_eq!(rebuilt, lengths);
    }

    #[test]
    fn repeats_shrink_the_stream() {
        let data = b"Kata ".repeat(10_000);
        let stream = zlib(&data);
        assert_eq!(stream[..2], [0x78, 0x9C]);
        assert!(stream.len() < 200, "{} bytes", stream.len());
    }
}
