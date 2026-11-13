//! DEFLATE compression (RFC 1951) inside a zlib stream (RFC 1950): how PNG stores
//! its pixels.
//!
//! The data is read in spans, as many at once as there are cores. Each span is
//! first read the quick way, taking the longest repeat at every step. What that
//! reading costs then prices each literal and each repeat, and the span is read
//! again for the cheapest way through it. The data comes in rows, a capture's, and
//! the row above is always looked at: what repeats is often right there. Blocks
//! end where the data changes, each with Huffman codes of its own, the shortest
//! that fit DEFLATE's limits, found by package-merge; a block too small to pay for
//! its codes uses DEFLATE's fixed ones.

use std::ops::Range;

use crate::parallel;

/// How far back a repeat can reach.
const WINDOW: usize = 1 << 15;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
/// Earlier positions a search tries before settling for the best so far.
const MAX_CHAIN: usize = 64;
/// Earlier positions a search tries when the answer is only weighed.
const QUICK_CHAIN: usize = 16;
/// A repeat this long ends the search.
const NICE_MATCH: usize = 128;
/// A repeat this long is taken whole: nothing inside it is weighed.
const WHOLE: usize = 258;
/// Positions are chained by their first four bytes: chaining by three finds more
/// short repeats, which cost about as much as the bytes they replace.
const HASHED: usize = 4;
const HASH_BITS: u32 = 15;
/// Bytes read as one piece of work, priced by what they hold themselves.
const SPAN: usize = 1 << 18;
/// How many times a span is read again, each time priced by the reading before.
const ROUNDS: usize = 3;
/// Tokens weighed together when deciding where a block ends.
const PIECE: usize = 1 << 8;
/// The most tokens a block holds.
const MOST_TOKENS: usize = 1 << 18;
/// Prices count this many parts of a bit.
const BIT: u32 = 32;
/// No earlier position.
const NONE: u32 = u32::MAX;

const END_OF_BLOCK: usize = 256;
/// The order in which a block lists its code-length code.
const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// `data` compressed into a zlib stream. It comes in rows `row` bytes long, or in
/// none if `row` is 0: a row often repeats the one above it.
pub fn zlib(data: &[u8], row: usize) -> Vec<u8> {
    let mut bits = Bits::default();
    // DEFLATE with a 32 KiB window; the second byte makes the pair a multiple of 31.
    bits.out.extend_from_slice(&[0x78, 0x9C]);
    let mut blocks = Blocks::new(&mut bits);
    let spans: Vec<Range<usize>> = (0..data.len())
        .step_by(SPAN)
        .map(|start| start..data.len().min(start + SPAN))
        .collect();
    // A wave at a time, so only as many spans as cores are ever held as tokens.
    for wave in spans.chunks(parallel::width()) {
        for tokens in parallel::each(wave.len(), |span| read(data, wave[span].clone(), row)) {
            blocks.feed(&tokens);
        }
    }
    blocks.finish();
    let mut out = bits.finish();
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// Roughly how many bits `data` compresses to, found the quick way: enough to tell
/// which of several ways of writing the same pixels compresses best. Its rows are
/// `row` bytes long.
pub fn weigh(data: &[u8], row: usize) -> u64 {
    let mut chains = Chains::new(data, 0..data.len(), row);
    let mut counts = Counts::default();
    let mut pos = 0;
    while pos < data.len() {
        let (mut length, mut distance) = chains.find(pos, QUICK_CHAIN);
        // The row above, where the search stops short of it.
        let above = chains.above(pos, 0);
        if above > length {
            (length, distance) = (above, row);
        }
        if length < MIN_MATCH {
            counts.add(Token::literal(data[pos]));
            pos += 1;
            continue;
        }
        counts.add(Token::repeat(length, distance));
        for skipped in pos + 1..pos + length {
            chains.insert(skipped);
        }
        pos += length;
    }
    counts.bits()
}

/// The tokens that write `span` of `data` in the fewest bits found. Its repeats are
/// looked up once; then it's read by taking the longest at each step, and read
/// again, each time for the cheapest way through at the prices of the reading
/// before.
fn read(data: &[u8], span: Range<usize>, row: usize) -> Vec<Token> {
    let repeats = Repeats::new(data, span.clone(), row);
    let mut counts = repeats.longest(data);
    let mut best: Option<(u64, Vec<Token>)> = None;
    for _ in 0..ROUNDS {
        let tokens = repeats.cheapest(data, &Prices::new(&counts));
        counts = Counts::of(&tokens);
        let weight = counts.bits();
        if best
            .as_ref()
            .is_some_and(|&(lightest, _)| lightest <= weight)
        {
            break;
        }
        best = Some((weight, tokens));
    }
    best.expect("a reading").1
}

/// The repeats that start at each byte of a span: for every length, the nearest
/// earlier place that repeats that much.
struct Repeats {
    span: Range<usize>,
    /// Where each byte's repeats start in `found`, and one past the last.
    starts: Vec<u32>,
    /// Lengths and distances, each repeat longer and farther than the one before.
    found: Vec<(u16, u16)>,
}

impl Repeats {
    fn new(data: &[u8], span: Range<usize>, row: usize) -> Self {
        let mut chains = Chains::new(data, span.clone(), row);
        let mut starts = Vec::with_capacity(span.len() + 1);
        let mut found = Vec::new();
        let mut pos = span.start;
        // How much of the byte before repeated the row above: all but one of those
        // bytes do here too, so comparing goes on from there.
        let mut above = 0usize;
        while pos < span.end {
            starts.push(found.len() as u32);
            let mut most = 0;
            chains.search(pos, MAX_CHAIN, |length, distance| {
                found.push((length as u16, distance as u16));
                most = length;
            });
            // The row above too: in a capture, what repeats is often right there,
            // farther back than the search gets where much else looks alike.
            above = chains.above(pos, above.saturating_sub(1));
            if above > most && above >= MIN_MATCH {
                found.push((above as u16, row as u16));
                most = above;
            }
            pos += 1;
            if most >= WHOLE {
                // So long a repeat is taken whole: nothing inside it is looked up.
                let whole = found.pop().expect("the longest");
                found.truncate(starts[starts.len() - 1] as usize);
                found.push(whole);
                for skipped in pos..pos + most - 1 {
                    chains.insert(skipped);
                    starts.push(found.len() as u32);
                }
                pos += most - 1;
                above = 0;
            }
        }
        starts.push(found.len() as u32);
        Self {
            span,
            starts,
            found,
        }
    }

    /// The repeats that start `at` bytes into the span.
    fn at(&self, at: usize) -> &[(u16, u16)] {
        &self.found[self.starts[at] as usize..self.starts[at + 1] as usize]
    }

    /// What the span holds, read by taking the longest repeat at each step.
    fn longest(&self, data: &[u8]) -> Counts {
        let mut counts = Counts::default();
        let mut at = 0;
        while at < self.span.len() {
            let token = match self.at(at).last() {
                Some(&(length, distance)) => Token { length, distance },
                None => Token::literal(data[self.span.start + at]),
            };
            counts.add(token);
            at += token.span();
        }
        counts
    }

    /// The span read the cheapest way at `prices`: of all the literals and repeats
    /// that lead to each byte, the ones that get there in the fewest bits.
    fn cheapest(&self, data: &[u8], prices: &Prices) -> Vec<Token> {
        let len = self.span.len();
        // The lowest price that reaches each byte, and the token that reaches it so.
        let mut cost = vec![u32::MAX; len + 1];
        let mut step = vec![Token::literal(0); len + 1];
        cost[0] = 0;
        let mut at = 0;
        while at < len {
            let here = cost[at];
            let mut reach = |to: usize, price: u32, token: Token| {
                if price < cost[to] {
                    cost[to] = price;
                    step[to] = token;
                }
            };
            let byte = data[self.span.start + at];
            reach(
                at + 1,
                here + prices.literals[usize::from(byte)],
                Token::literal(byte),
            );
            let repeats = self.at(at);
            if let [(length, distance)] = *repeats
                && usize::from(length) >= WHOLE
            {
                let price = here + prices.lengths[usize::from(length)] + prices.distance(distance);
                reach(at + usize::from(length), price, Token { length, distance });
                at += usize::from(length);
                continue;
            }
            // Each length goes to the nearest place that repeats that much.
            let mut length = MIN_MATCH as u16;
            for &(most, distance) in repeats {
                let far = here + prices.distance(distance);
                while length <= most {
                    let price = far + prices.lengths[usize::from(length)];
                    reach(at + usize::from(length), price, Token { length, distance });
                    length += 1;
                }
            }
            at += 1;
        }

        let mut tokens = Vec::new();
        let mut at = len;
        while at > 0 {
            tokens.push(step[at]);
            at -= step[at].span();
        }
        tokens.reverse();
        tokens
    }
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

    /// How many bytes it writes.
    fn span(self) -> usize {
        if self.distance == 0 {
            1
        } else {
            usize::from(self.length)
        }
    }
}

/// How often each symbol of a block's two alphabets is used.
#[derive(Clone)]
struct Counts {
    literals: [u32; 286],
    distances: [u32; 30],
}

impl Default for Counts {
    fn default() -> Self {
        Self {
            literals: [0; 286],
            distances: [0; 30],
        }
    }
}

impl Counts {
    fn of(tokens: &[Token]) -> Self {
        let mut counts = Self::default();
        for &token in tokens {
            counts.add(token);
        }
        counts
    }

    fn add(&mut self, token: Token) {
        if token.distance == 0 {
            self.literals[usize::from(token.length)] += 1;
        } else {
            self.literals[257 + length_symbol(token.length).0] += 1;
            self.distances[distance_symbol(token.distance).0] += 1;
        }
    }

    /// These and `other` together.
    fn with(&self, other: &Self) -> Self {
        let mut both = self.clone();
        for (count, more) in both.literals.iter_mut().zip(other.literals) {
            *count += more;
        }
        for (count, more) in both.distances.iter_mut().zip(other.distances) {
            *count += more;
        }
        both
    }

    /// Roughly the bits a block of these takes: each symbol at the price its share
    /// earns it, never under a bit, plus the extra bits and the codes themselves.
    fn bits(&self) -> u64 {
        fn priced(counts: &[u32], extra: impl Fn(usize) -> u32) -> f32 {
            let total = counts.iter().sum::<u32>() as f32;
            let symbols = counts.iter().enumerate().filter(|&(_, &count)| count > 0);
            symbols
                .map(|(symbol, &count)| {
                    let count = count as f32;
                    // About four bits say how long each symbol's code is.
                    4.0 + count * ((total / count).log2().max(1.0) + extra(symbol) as f32)
                })
                .sum()
        }
        let literals = priced(&self.literals, |symbol| match symbol {
            265..285 => (symbol as u32 - 261) / 4,
            _ => 0,
        });
        let distances = priced(&self.distances, |symbol| {
            (symbol as u32).saturating_sub(2) / 2
        });
        74 + (literals + distances) as u64
    }
}

/// What each literal and each repeat costs, in `BIT`s, going by how often a reading
/// used them.
struct Prices {
    literals: [u32; 256],
    /// By length, extra bits included.
    lengths: [u32; MAX_MATCH + 1],
    /// By distance symbol, extra bits included.
    distances: [u32; 30],
}

impl Prices {
    fn new(counts: &Counts) -> Self {
        fn price(count: u32, total: f32) -> u32 {
            // A symbol never seen costs as if it had been seen half a time.
            let share = (count as f32).max(0.5) / total.max(1.0);
            (BIT as f32 * -share.log2()).clamp(BIT as f32, 15.0 * BIT as f32) as u32
        }
        let total = counts.literals.iter().sum::<u32>() as f32;
        let far = counts.distances.iter().sum::<u32>() as f32;
        let mut prices = Self {
            literals: [0; 256],
            lengths: [0; MAX_MATCH + 1],
            distances: [0; 30],
        };
        for (literal, &count) in prices.literals.iter_mut().zip(&counts.literals) {
            *literal = price(count, total);
        }
        for (length, cost) in prices.lengths.iter_mut().enumerate().skip(MIN_MATCH) {
            let (symbol, extra_bits, _) = length_symbol(length as u16);
            *cost = price(counts.literals[257 + symbol], total) + BIT * extra_bits;
        }
        for (symbol, distance) in prices.distances.iter_mut().enumerate() {
            let extra_bits = (symbol as u32).saturating_sub(2) / 2;
            *distance = price(counts.distances[symbol], far) + BIT * extra_bits;
        }
        prices
    }

    fn distance(&self, distance: u16) -> u32 {
        self.distances[distance_symbol(distance).0]
    }
}

/// Every earlier position, chained by the hash of the bytes starting there.
struct Chains<'a> {
    data: &'a [u8],
    /// How long a row is, or 0 if the data has none.
    row: usize,
    /// Where repeats have to end.
    end: usize,
    /// The latest position with each hash.
    head: Vec<u32>,
    /// The position before each one with the same hash, within the window.
    prev: Vec<u32>,
}

impl<'a> Chains<'a> {
    /// The chains to read `span` with, of data in rows `row` bytes long: what comes
    /// just before it is already in.
    fn new(data: &'a [u8], span: Range<usize>, row: usize) -> Self {
        let mut chains = Self {
            data,
            // A row too long to reach back to has none above it, as far as repeats go.
            row: if row <= WINDOW { row } else { 0 },
            end: span.end,
            head: vec![NONE; 1 << HASH_BITS],
            prev: vec![NONE; WINDOW],
        };
        for pos in span.start.saturating_sub(WINDOW)..span.start {
            chains.insert(pos);
        }
        chains
    }

    /// How much of what starts at `pos` repeats the row above, knowing that its
    /// first `known` bytes do.
    fn above(&self, pos: usize, known: usize) -> usize {
        if self.row == 0 || pos < self.row {
            return 0;
        }
        let max = (self.end - pos).min(MAX_MATCH);
        let known = known.min(max);
        let (earlier, pos) = (pos - self.row + known, pos + known);
        known + match_length(self.data, earlier, pos, max - known)
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
        let mut best = (0, 0);
        self.search(pos, chain, |length, distance| best = (length, distance));
        best
    }

    /// Tries the earlier positions chained to `pos`, nearest first, and calls
    /// `longer` with each repeat that beats those before it, as length and distance.
    /// Adds `pos` for later searches.
    fn search(&mut self, pos: usize, chain: usize, mut longer: impl FnMut(usize, usize)) {
        let data = self.data;
        let max = (self.end - pos).min(MAX_MATCH);
        if max < HASHED {
            return;
        }
        let hash = self.hash(pos);
        let nice = max.min(NICE_MATCH);
        let word = |at: usize| u32::from_le_bytes(data[at..at + HASHED].try_into().unwrap());
        let mut best = MIN_MATCH - 1;
        // Only a repeat that reaches past the best so far can beat it: its four bytes
        // up to there have to be these.
        let mut tail = best - (HASHED - 1).min(best);
        let mut wanted = word(pos + tail);
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
            if word(start + tail) == wanted {
                let length = match_length(data, start, pos, max);
                if length > best {
                    best = length;
                    longer(length, distance);
                    if length >= nice {
                        break;
                    }
                    tail = best - (HASHED - 1).min(best);
                    wanted = word(pos + tail);
                }
            }
            candidate = self.prev[start & (WINDOW - 1)];
        }
        self.link(pos, hash);
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

/// Tokens on their way to the stream, cut into blocks where what they hold changes.
struct Blocks<'a> {
    bits: &'a mut Bits,
    /// The block being gathered, with its counts and their weight.
    block: Vec<Token>,
    counts: Counts,
    weight: u64,
}

impl<'a> Blocks<'a> {
    fn new(bits: &'a mut Bits) -> Self {
        Self {
            bits,
            block: Vec::new(),
            counts: Counts::default(),
            weight: 0,
        }
    }

    /// Adds `tokens`, a piece at a time: a piece joins the block unless the two
    /// weigh less apart, each with codes of its own.
    fn feed(&mut self, tokens: &[Token]) {
        for piece in tokens.chunks(PIECE) {
            let counts = Counts::of(piece);
            let apart = counts.bits();
            let joined = self.counts.with(&counts);
            let together = joined.bits();
            let full = self.block.len() + piece.len() > MOST_TOKENS;
            if !self.block.is_empty() && (full || together > self.weight + apart) {
                write_block(&self.block, false, self.bits);
                self.block.clear();
                (self.counts, self.weight) = (counts, apart);
            } else {
                (self.counts, self.weight) = (joined, together);
            }
            self.block.extend_from_slice(piece);
        }
    }

    fn finish(self) {
        write_block(&self.block, true, self.bits);
    }
}

/// Writes `tokens` as one block, with Huffman codes made for it or with DEFLATE's
/// fixed ones, whichever takes fewer bits.
fn write_block(tokens: &[Token], last: bool, bits: &mut Bits) {
    let mut counts = Counts::of(tokens);
    counts.literals[END_OF_BLOCK] = 1;
    let own = Codes::fitted(&counts);
    let header = Header::new(&own);
    let fixed = Codes::fixed();
    bits.put(u32::from(last), 1);
    if header.bits() + own.bits(&counts) < fixed.bits(&counts) {
        bits.put(2, 2);
        header.put(bits);
        own.put(tokens, bits);
    } else {
        bits.put(1, 2);
        fixed.put(tokens, bits);
    }
}

/// The two codes of a block: one for literals and lengths, one for distances.
struct Codes {
    literals: Code,
    distances: Code,
}

impl Codes {
    /// The codes that write symbols seen `counts` times in the fewest bits.
    fn fitted(counts: &Counts) -> Self {
        Self {
            literals: Code::new(code_lengths(&counts.literals, 15)),
            distances: Code::new(code_lengths(&counts.distances, 15)),
        }
    }

    /// The codes DEFLATE has ready, which no block has to spell out.
    fn fixed() -> Self {
        let literals = (0..288).map(|symbol| match symbol {
            0..144 => 8,
            144..256 => 9,
            256..280 => 7,
            _ => 8,
        });
        Self {
            literals: Code::new(literals.collect()),
            distances: Code::new(vec![5; 30]),
        }
    }

    /// The bits symbols seen `counts` times take, leaving out the extra bits, which
    /// are the same with any code.
    fn bits(&self, counts: &Counts) -> u64 {
        let bits = |counts: &[u32], code: &Code| -> u64 {
            let lengths = counts.iter().zip(&code.lengths);
            lengths
                .map(|(&count, &length)| u64::from(count) * u64::from(length))
                .sum()
        };
        bits(&counts.literals, &self.literals) + bits(&counts.distances, &self.distances)
    }

    /// Writes `tokens`, then the end of the block.
    fn put(&self, tokens: &[Token], bits: &mut Bits) {
        for token in tokens {
            if token.distance == 0 {
                self.literals.put(bits, usize::from(token.length));
            } else {
                let (symbol, extra_bits, extra) = length_symbol(token.length);
                self.literals.put(bits, 257 + symbol);
                bits.put(extra, extra_bits);
                let (symbol, extra_bits, extra) = distance_symbol(token.distance);
                self.distances.put(bits, symbol);
                bits.put(extra, extra_bits);
            }
        }
        self.literals.put(bits, END_OF_BLOCK);
    }
}

/// How a block spells out its codes: both codes' lengths, trimmed of unused symbols
/// at the end, run-length encoded with a code of their own.
struct Header {
    literal_len: usize,
    distance_len: usize,
    order_len: usize,
    run_code: Code,
    runs: Vec<(u8, u8)>,
}

impl Header {
    fn new(codes: &Codes) -> Self {
        let literal_len = used(&codes.literals.lengths).max(257);
        let distance_len = used(&codes.distances.lengths).max(1);
        let lengths: Vec<u8> = codes.literals.lengths[..literal_len]
            .iter()
            .chain(&codes.distances.lengths[..distance_len])
            .copied()
            .collect();
        let runs = runs(&lengths);
        let mut run_counts = [0; 19];
        for &(symbol, _) in &runs {
            run_counts[usize::from(symbol)] += 1;
        }
        let run_code = Code::new(code_lengths(&run_counts, 7));
        let order_len = CODE_LENGTH_ORDER
            .iter()
            .rposition(|&symbol| run_code.lengths[symbol] != 0)
            .map_or(0, |last| last + 1)
            .max(4);
        Self {
            literal_len,
            distance_len,
            order_len,
            run_code,
            runs,
        }
    }

    fn bits(&self) -> u64 {
        let runs = self.runs.iter().map(|&(symbol, _)| {
            u64::from(self.run_code.lengths[usize::from(symbol)]) + u64::from(run_extra(symbol))
        });
        14 + 3 * self.order_len as u64 + runs.sum::<u64>()
    }

    fn put(&self, bits: &mut Bits) {
        bits.put((self.literal_len - 257) as u32, 5);
        bits.put((self.distance_len - 1) as u32, 5);
        bits.put((self.order_len - 4) as u32, 4);
        for &symbol in &CODE_LENGTH_ORDER[..self.order_len] {
            bits.put(u32::from(self.run_code.lengths[symbol]), 3);
        }
        for &(symbol, extra) in &self.runs {
            self.run_code.put(bits, usize::from(symbol));
            bits.put(u32::from(extra), run_extra(symbol));
        }
    }
}

/// How many extra bits follow a code-length symbol.
fn run_extra(symbol: u8) -> u32 {
    match symbol {
        16 => 2,
        17 => 3,
        18 => 7,
        _ => 0,
    }
}

/// A canonical Huffman code.
struct Code {
    lengths: Vec<u8>,
    /// Each symbol's code, bit-reversed: DEFLATE writes codes first bit first.
    codes: Vec<u32>,
}

impl Code {
    /// The code whose symbols are this many bits long.
    fn new(lengths: Vec<u8>) -> Self {
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
        let stream = zlib(&data, 0);
        assert_eq!(stream[..2], [0x78, 0x9C]);
        assert!(stream.len() < 200, "{} bytes", stream.len());
    }
}
