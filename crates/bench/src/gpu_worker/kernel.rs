//! The GPU compute kernel (WGSL), its FLOP accounting and the CPU reference used to
//! verify the output. Pure: unit-tested without a GPU.
//!
//! Each invocation `i`:
//! 1. seeds `CHAINS` independent `vec4<f32>` accumulators from a hash of `(i, lane)`
//!    (values in [0, 1));
//! 2. runs `ITERS` iterations of `a = fma(a, M, C)` on every chain — the chains are
//!    independent so the GPU can overlap them (ILP); `M` < 1 makes the map contractive, so
//!    values stay finite and rounding differences cannot grow;
//! 3. writes the sum of all lanes to `out[i]`.
//!
//! **FLOP count** (only the FMAs are counted; one FMA = 2 FLOP):
//! `CHAINS × 4 lanes × ITERS × 2` = 8 × 4 × 512 × 2 = **32 768 FLOP per invocation**.
//! The seeding (integer hash) and the final 31 additions per invocation are not counted,
//! so the figure is a slight underestimate of the work actually done.
//!
//! Changing anything here changes the result: bump `methodology_version(GpuCompute)`.

pub(crate) const WORKGROUP_SIZE: u32 = 256;
pub(crate) const CHAINS: u32 = 8;
pub(crate) const LANES: u32 = 4;
pub(crate) const ITERS: u32 = 512;
pub(crate) const FLOPS_PER_FMA: u64 = 2;

/// Counted FLOPs of one invocation (see the module docs).
pub(crate) const fn flops_per_invocation() -> u64 {
    CHAINS as u64 * LANES as u64 * ITERS as u64 * FLOPS_PER_FMA
}

/// FLOPs of `dispatches` dispatches of `workgroups` workgroups each.
pub(crate) fn flops_for(workgroups: u32, dispatches: u64) -> f64 {
    f64::from(workgroups)
        * f64::from(WORKGROUP_SIZE)
        * dispatches as f64
        * flops_per_invocation() as f64
}

/// Per-lane multipliers and addends (lane 0..3), shared by WGSL and the CPU reference.
pub(crate) const M: [f32; 4] = [0.9990, 0.9992, 0.9994, 0.9996];
pub(crate) const C: [f32; 4] = [0.0010, 0.0007, 0.0004, 0.0001];

pub(crate) fn wgsl() -> String {
    let chains = CHAINS;
    let mut seeds = String::new();
    let mut steps = String::new();
    let mut sum = String::from("vec4<f32>(0.0)");
    for k in 0..chains {
        let b = k * LANES;
        seeds.push_str(&format!(
            "    var a{k} = vec4<f32>(seed(i, {}u), seed(i, {}u), seed(i, {}u), seed(i, {}u));\n",
            b,
            b + 1,
            b + 2,
            b + 3
        ));
        steps.push_str(&format!("        a{k} = fma(a{k}, m, c);\n"));
        sum = format!("({sum} + a{k})");
    }
    format!(
        r"// PC Diagnostics GPU compute kernel (methodology v1). See kernel.rs.
@group(0) @binding(0) var<storage, read_write> out_buf: array<f32>;

fn seed(i: u32, k: u32) -> f32 {{
    var x = i * 747796405u + k * 2891336453u + 1u;
    x = ((x >> ((x >> 28u) + 4u)) ^ x) * 277803737u;
    x = (x >> 22u) ^ x;
    return f32(x & 0xFFFFu) / 65536.0;
}}

@compute @workgroup_size({wg})
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {{
    let i = gid.x;
    let m = vec4<f32>({m0:?}, {m1:?}, {m2:?}, {m3:?});
    let c = vec4<f32>({c0:?}, {c1:?}, {c2:?}, {c3:?});
{seeds}    for (var it = 0u; it < {iters}u; it = it + 1u) {{
{steps}    }}
    let s = {sum};
    out_buf[i] = (s.x + s.y) + (s.z + s.w);
}}
",
        wg = WORKGROUP_SIZE,
        m0 = M[0],
        m1 = M[1],
        m2 = M[2],
        m3 = M[3],
        c0 = C[0],
        c1 = C[1],
        c2 = C[2],
        c3 = C[3],
        iters = ITERS,
    )
}

/// CPU copy of the WGSL `seed` (u32 wrapping arithmetic, as WGSL defines it).
pub(crate) fn seed(i: u32, k: u32) -> f32 {
    let mut x = i
        .wrapping_mul(747_796_405)
        .wrapping_add(k.wrapping_mul(2_891_336_453))
        .wrapping_add(1);
    x = ((x >> ((x >> 28) + 4)) ^ x).wrapping_mul(277_803_737);
    x ^= x >> 22;
    (x & 0xFFFF) as f32 / 65536.0
}

/// CPU reference of `out[i]`.
pub(crate) fn reference(i: u32) -> f32 {
    let mut s = [0.0f32; 4];
    for k in 0..CHAINS {
        let mut a = [0.0f32; 4];
        for (l, v) in a.iter_mut().enumerate() {
            *v = seed(i, k * LANES + l as u32);
        }
        for _ in 0..ITERS {
            for l in 0..4 {
                a[l] = a[l].mul_add(M[l], C[l]);
            }
        }
        for l in 0..4 {
            s[l] += a[l];
        }
    }
    (s[0] + s[1]) + (s[2] + s[3])
}

/// Relative tolerance of the comparison. The map is contractive (|M| ≤ 0.9996), so the
/// accumulated rounding difference between fused/unfused FMA and summation order is
/// bounded by ~eps/(1 − M) ≈ 1.5e-4 relative; 2e-3 leaves margin while still catching
/// any wrong, missing or garbage result (values are O(1)).
pub(crate) const REL_TOLERANCE: f64 = 2e-3;

/// Indices checked after the run: both ends, workgroup boundaries and a spread in
/// between, all `< invocations`.
pub(crate) fn check_indices(invocations: u32) -> Vec<u32> {
    let n = invocations.max(1);
    let mut v = vec![
        0,
        1,
        WORKGROUP_SIZE - 1,
        WORKGROUP_SIZE,
        n / 2,
        n.saturating_sub(2),
        n - 1,
    ];
    let mut x = 0x9E37_79B9u32;
    while v.len() < 64 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        v.push(x % n);
    }
    v.retain(|i| *i < n);
    v.sort_unstable();
    v.dedup();
    v
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Mismatch {
    pub index: u32,
    pub gpu: f32,
    pub cpu: f32,
}

/// Compares GPU values with the CPU reference. `Ok(max relative error)`.
pub(crate) fn verify(indices: &[u32], gpu: &[f32]) -> Result<f64, Mismatch> {
    let mut max_rel = 0.0f64;
    for (&index, &g) in indices.iter().zip(gpu) {
        let cpu = reference(index);
        let rel = (f64::from(g) - f64::from(cpu)).abs() / f64::from(cpu).abs().max(1e-6);
        if !g.is_finite() || rel.is_nan() || rel > REL_TOLERANCE {
            return Err(Mismatch { index, gpu: g, cpu });
        }
        max_rel = max_rel.max(rel);
    }
    if gpu.len() < indices.len() {
        return Err(Mismatch {
            index: indices[gpu.len()],
            gpu: f32::NAN,
            cpu: reference(indices[gpu.len()]),
        });
    }
    Ok(max_rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flop_accounting() {
        assert_eq!(flops_per_invocation(), 32_768);
        // One dispatch of 16 384 workgroups × 256 invocations.
        let f = flops_for(16_384, 1);
        assert_eq!(f, 16_384.0 * 256.0 * 32_768.0);
        assert_eq!(flops_for(64, 10), 10.0 * 64.0 * 256.0 * 32_768.0);
        // The WGSL really contains CHAINS FMA statements per iteration and ITERS iterations.
        let src = wgsl();
        assert_eq!(src.matches("= fma(").count(), CHAINS as usize);
        assert!(src.contains(&format!("it < {ITERS}u")));
        assert!(src.contains(&format!("@workgroup_size({WORKGROUP_SIZE})")));
    }

    #[test]
    fn reference_is_deterministic_and_bounded() {
        let a = reference(12_345);
        assert_eq!(a, reference(12_345));
        assert!(a.is_finite());
        // Each lane converges toward C/(1-M) ≤ 1 from [0,1): the sum stays below 32.
        assert!(a > 0.0 && a < 32.0, "{a}");
        assert_ne!(reference(0), reference(1));
        assert!((0.0..1.0).contains(&seed(7, 3)));
    }

    #[test]
    fn checksum_verification() {
        let idx = check_indices(1 << 20);
        assert!(idx.len() >= 32);
        assert!(idx.iter().all(|i| *i < (1 << 20)));
        assert!(idx.contains(&0) && idx.contains(&((1 << 20) - 1)));
        let good: Vec<f32> = idx.iter().map(|&i| reference(i)).collect();
        assert_eq!(verify(&idx, &good), Ok(0.0));
        // Within tolerance (e.g. unfused FMA on the GPU).
        let close: Vec<f32> = good.iter().map(|v| v * (1.0 + 1e-4)).collect();
        assert!(verify(&idx, &close).is_ok());
        // Wrong, zero (kernel never ran) and NaN outputs fail.
        let mut bad = good.clone();
        bad[3] *= 1.01;
        assert_eq!(verify(&idx, &bad).expect_err("mismatch").index, idx[3]);
        assert!(verify(&idx, &vec![0.0; idx.len()]).is_err());
        let mut nan = good.clone();
        nan[0] = f32::NAN;
        assert!(verify(&idx, &nan).is_err());
        // Missing values fail.
        assert!(verify(&idx, &good[..2]).is_err());
        // Tiny dispatches keep indices in range.
        assert!(check_indices(256).iter().all(|i| *i < 256));
    }
}
