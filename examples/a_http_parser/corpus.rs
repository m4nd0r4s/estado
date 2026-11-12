//! Deterministic request streams for the differential test and BM-01.

/// A stream of well-formed requests.
#[derive(Debug, Clone)]
pub struct Corpus {
    /// The bytes of all requests, back to back.
    pub bytes: Vec<u8>,
    /// How many requests `bytes` holds.
    pub requests: u32,
}

/// Small deterministic generator (xorshift), so streams are reproducible.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

const METHODS: [&str; 5] = ["GET", "POST", "PUT", "DELETE", "HEAD"];
const PATH_WORDS: [&str; 8] = [
    "api", "v1", "users", "items", "search", "static", "img", "orders",
];
const HEADERS: [(&str, &str); 8] = [
    ("Host", "example.org"),
    (
        "User-Agent",
        "Mozilla/5.0 (X11; Linux x86_64) Gecko/20100101",
    ),
    ("Accept", "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8"),
    ("Accept-Language", "en-US,en;q=0.5"),
    ("Accept-Encoding", "gzip, deflate"),
    ("Connection", "keep-alive"),
    ("Cache-Control", "max-age=0"),
    ("Cookie", "session=0123456789abcdef; theme=dark"),
];

fn push_line(bytes: &mut Vec<u8>, text: &str) {
    bytes.extend_from_slice(text.as_bytes());
    bytes.extend_from_slice(b"\r\n");
}

fn push_request(bytes: &mut Vec<u8>, random: &mut Random) {
    let method = METHODS[random.below(METHODS.len() as u64) as usize];
    let mut target = String::new();
    for _ in 0..1 + random.below(5) {
        target.push('/');
        target.push_str(PATH_WORDS[random.below(PATH_WORDS.len() as u64) as usize]);
    }
    if random.below(2) == 0 {
        target.push_str(&format!(
            "?id={}&page={}",
            random.below(100_000),
            random.below(50)
        ));
    }
    push_line(bytes, &format!("{method} {target} HTTP/1.1"));

    let has_body = method == "POST" || method == "PUT";
    let body_length = if has_body { random.below(1500) } else { 0 };
    let header_count = 3 + random.below(6) as usize;
    let length_position = random.below(header_count as u64) as usize;
    for position in 0..header_count {
        if has_body && position == length_position {
            let name = if random.below(2) == 0 {
                "Content-Length"
            } else {
                "content-length"
            };
            push_line(bytes, &format!("{name}: {body_length}"));
        } else {
            let (name, value) = HEADERS[random.below(HEADERS.len() as u64) as usize];
            push_line(bytes, &format!("{name}: {value}"));
        }
    }
    bytes.extend_from_slice(b"\r\n");
    for _ in 0..body_length {
        bytes.push(b'a' + random.below(26) as u8);
    }
}

/// Well-formed requests (GET, POST with bodies, ...) until `bytes.len()` is
/// at least `minimum_bytes`.
pub fn build_requests(minimum_bytes: usize, seed: u64) -> Corpus {
    let mut random = Random(seed | 1);
    let mut bytes = Vec::with_capacity(minimum_bytes + 4096);
    let mut requests = 0;
    while bytes.len() < minimum_bytes {
        push_request(&mut bytes, &mut random);
        requests += 1;
    }
    Corpus { bytes, requests }
}

/// `bytes` with `flips` random bytes replaced by random ones, to reach the
/// error paths.
pub fn corrupt(bytes: &[u8], flips: usize, seed: u64) -> Vec<u8> {
    let mut random = Random(seed | 1);
    let mut corrupted = bytes.to_vec();
    for _ in 0..flips {
        let position = random.below(corrupted.len() as u64) as usize;
        corrupted[position] = random.below(256) as u8;
    }
    corrupted
}
