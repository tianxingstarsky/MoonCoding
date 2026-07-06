use std::collections::HashMap;

/// 极简 char-gram TF-IDF 余弦相似度(零依赖, 零模型).
/// 仅作"功能说明严重漂移"的兜底 WARN, 不阻断任何操作.
/// 目标: 抓住 AI 完全没动 purpose 的大头漂移; 不负责捕捉同义改写.

const N: usize = 3;

fn ngrams(text: &str) -> HashMap<String, f64> {
    let s = text.to_lowercase();
    let chars: Vec<char> = s.chars().collect();
    let mut m: HashMap<String, f64> = HashMap::new();
    if chars.len() < N {
        *m.entry(s).or_insert(0.0) += 1.0;
        return m;
    }
    for w in chars.windows(N) {
        let k: String = w.iter().collect();
        *m.entry(k).or_insert(0.0) += 1.0;
    }
    m
}

fn norm(v: &HashMap<String, f64>) -> f64 {
    v.values().map(|x| x * x).sum::<f64>().sqrt()
}

pub fn cosine(a: &str, b: &str) -> f64 {
    let va = ngrams(a);
    let vb = ngrams(b);
    let na = norm(&va);
    let nb = norm(&vb);
    if na == 0.0 || nb == 0.0 { return 0.0; }
    let mut dot = 0.0;
    let (small, big) = if va.len() < vb.len() { (&va, &vb) } else { (&vb, &va) };
    for (k, v) in small.iter() {
        if let Some(w) = big.get(k) { dot += v * w; }
    }
    dot / (na * nb)
}

pub const THRESHOLD: f64 = 0.6;

pub fn check_drift(purpose: &str, breakdown: &[String]) -> (bool, f64) {
    let joined = breakdown.join("  ");
    let sim = cosine(purpose, &joined);
    (sim < THRESHOLD, sim)
}