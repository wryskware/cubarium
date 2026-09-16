//! The cube's outputs, captured by value before `Topology` existed.
//!
//! This file is FW-1's regression evidence (`design/flat-world-plan-2026-09-16.md` §9):
//! the surface crate sits below `cubarium-core`, so it cannot build the `CubeProjection`
//! the later packages compare on. Instead it dumps `embed`, `chord_sq`, `travel`,
//! `unfold`, `unfold_pixels`, the `FieldGraph` and `deposit` over a fixed fixture set,
//! as exact IEEE-754 bit patterns, and compares the dump with a golden file written
//! from the tree as it stood before the topology work started.
//!
//! Bulk sections are folded into a 64-bit FNV-1a digest so the golden stays small; a
//! few representative rows are written out in full so a mismatch is diagnosable. Widening
//! a signature must change the call sites in this file and nothing in `data/cube_by_value.txt`.
//!
//! Regenerate deliberately (and only with a reviewed reason) with
//! `CUBARIUM_UPDATE_GOLDEN=1 cargo test -p cubarium-surface --test cube_by_value`.

use cubarium_surface::{Scale, Topology};
use cubarium_surface::{
    CUBE_CELL_COUNT, CellId, Edge, Face, FieldGraph, PixelImage, ScalarField, SurfacePoint, Travel,
    Unfolded, Vec2, cell_of, chart_images, deposit, diffuse, travel, unfold, unfold_pixels,
};

const GOLDEN: &str = include_str!("data/cube_by_value.txt");

/// Collects the dump: `row` is kept verbatim in the golden, `bulk` only feeds the digest.
struct Dump {
    text: String,
    hash: u64,
}

impl Dump {
    fn new() -> Dump {
        Dump { text: String::new(), hash: 0xcbf2_9ce4_8422_2325 }
    }

    fn feed(&mut self, s: &str) {
        for b in s.as_bytes() {
            self.hash ^= u64::from(*b);
            self.hash = self.hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.hash ^= 0xff;
        self.hash = self.hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    /// A line that appears in the golden and in the digest.
    fn row(&mut self, s: &str) {
        self.feed(s);
        self.text.push_str(s);
        self.text.push('\n');
    }

    /// A line that only feeds the digest.
    fn bulk(&mut self, s: &str) {
        self.feed(s);
    }

    /// Close a section: emit its digest as a row and reset.
    fn seal(&mut self, name: &str) {
        let h = self.hash;
        self.row(&format!("{name} digest {h:016x}"));
        self.hash = 0xcbf2_9ce4_8422_2325;
    }
}

/// Exact f64, as its IEEE-754 bit pattern: "identical by value" means bit-identical.
fn f(x: f64) -> String {
    format!("{:016x}", x.to_bits())
}

fn v(p: Vec2) -> String {
    format!("({},{})", f(p.x), f(p.y))
}

fn sp(p: &SurfacePoint) -> String {
    format!("{}:{}:{}", p.face.index(), f(p.u), f(p.v))
}

fn travel_row(t: &Travel) -> String {
    let mut s = format!(
        "end={} map={:?} x={} r={} ties={} fb={} segs={}",
        sp(&t.end),
        t.map.m,
        t.crossings,
        t.reflections,
        t.ties,
        t.fallback,
        t.segments.len()
    );
    for g in &t.segments {
        s.push_str(&format!(" [{} {}->{}]", g.face.index(), v(g.from), v(g.to)));
    }
    s
}

fn unfolded_row(u: &Unfolded) -> String {
    format!(
        "local={} map={:?} d={} path={}{:?}",
        v(u.local),
        u.map.m,
        f(u.distance),
        u.path.len,
        u.path.steps()
    )
}

fn pixel_row(p: &PixelImage) -> String {
    format!(
        "{}({},{}) local={} d={} path={}{:?}",
        p.face.index(),
        p.x,
        p.y,
        v(p.local),
        f(p.distance),
        p.path.len,
        p.path.steps()
    )
}

/// The xorshift the crate's own property tests use, so the fixture set is reproducible.
struct Rng(u64);

impl Rng {
    fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn point(&mut self) -> SurfacePoint {
        let face = Face::ALL[(self.next_f64() * 5.0) as usize % 5];
        SurfacePoint::new(face, self.next_f64() * 64.0, self.next_f64() * 64.0).canonicalize(Topology::Cube)
    }

    fn disp(&mut self, reach: f64) -> Vec2 {
        Vec2::new((self.next_f64() - 0.5) * reach, (self.next_f64() - 0.5) * reach)
    }
}

/// The named geometry fixtures: the design examples, both corner rules, the rim, and
/// the multi-chart overshoot.
const NAMED_TRAVEL: &[(Face, f64, f64, f64, f64)] = &[
    (Face::Front, 63.75, 20.0, 0.5, 0.0),
    (Face::Right, 10.0, 0.25, 0.0, -0.5),
    (Face::Back, 10.0, 0.25, 0.0, -0.5),
    (Face::Front, 63.0, 1.0, 2.0, -2.0),
    (Face::Front, 63.0, 63.0, 2.0, 2.0),
    (Face::Front, 10.0, 63.0, 0.0, 2.0),
    (Face::Front, 32.0, 32.0, 200.0, 0.0),
    (Face::Top, 0.5, 0.5, -3.0, -3.0),
    (Face::Top, 63.5, 63.5, 3.0, 3.0),
    (Face::Left, 0.25, 0.25, -1.0, -1.0),
];

const ANCHORS: &[(Face, f64, f64)] = &[
    (Face::Front, 32.5, 32.5),
    (Face::Front, 0.25, 0.25),
    (Face::Front, 63.75, 0.25),
    (Face::Front, 63.75, 63.75),
    (Face::Top, 0.5, 0.5),
    (Face::Top, 63.5, 63.5),
    (Face::Top, 32.0, 0.0),
    (Face::Right, 0.5, 0.5),
    (Face::Back, 63.5, 2.5),
    (Face::Left, 3.5, 61.5),
];

fn dump() -> String {
    let mut d = Dump::new();

    // --- embed: every pixel centre on every chart, plus the named corners in full.
    for face in Face::ALL {
        for y in 0..64u16 {
            for x in 0..64u16 {
                let p = SurfacePoint::pixel_center(Topology::Cube, face, x, y);
                let e = Topology::Cube.embed(Scale::ONE, &p);
                d.bulk(&format!("{}({x},{y}) {} {} {}", face.index(), f(e[0]), f(e[1]), f(e[2])));
            }
        }
    }
    for face in Face::ALL {
        for &(u, w) in &[(0.5f64, 0.5f64), (63.5, 0.5), (0.5, 63.5), (63.5, 63.5), (32.0, 32.0)] {
            let p = SurfacePoint::new(face, u, w);
            let e = Topology::Cube.embed(Scale::ONE, &p);
            d.row(&format!("embed {} -> {} {} {}", sp(&p), f(e[0]), f(e[1]), f(e[2])));
            let t = Topology::Cube.embed_tangent(Scale::ONE, &p, Vec2::new(1.0, -2.0));
            d.row(&format!("embed_tangent {} -> {} {} {}", sp(&p), f(t[0]), f(t[1]), f(t[2])));
        }
    }
    d.seal("embed");

    // --- chord_sq over a fixed pair set.
    let mut rng = Rng(0x1357_9bdf_2468_ace0);
    for i in 0..4000 {
        let a = rng.point();
        let b = rng.point();
        let c = Topology::Cube.chord_sq(&a, &b);
        if i < 8 {
            d.row(&format!("chord_sq {} {} -> {}", sp(&a), sp(&b), f(c)));
        } else {
            d.bulk(&format!("chord_sq {} {} -> {}", sp(&a), sp(&b), f(c)));
        }
    }
    d.seal("chord_sq");

    // --- travel: the named fixtures in full, then a broad swept set.
    for &(face, u, w, dx, dy) in NAMED_TRAVEL {
        let start = SurfacePoint::new(face, u, w);
        let t = travel(Topology::Cube, start, Vec2::new(dx, dy));
        d.row(&format!("travel {} + {} -> {}", sp(&start), v(Vec2::new(dx, dy)), travel_row(&t)));
    }
    // Every connected half-edge at every pixel-centre parameter.
    for face in Face::ALL {
        for edge in Edge::ALL {
            for k in 0..64u16 {
                let s = f64::from(k) + 0.5;
                let (start, disp) = match edge {
                    Edge::Top => (Vec2::new(s, 0.5), Vec2::new(0.0, -1.0)),
                    Edge::Right => (Vec2::new(63.5, s), Vec2::new(1.0, 0.0)),
                    Edge::Bottom => (Vec2::new(s, 63.5), Vec2::new(0.0, 1.0)),
                    Edge::Left => (Vec2::new(0.5, s), Vec2::new(-1.0, 0.0)),
                };
                let t = travel(Topology::Cube, SurfacePoint::new(face, start.x, start.y), disp);
                d.bulk(&travel_row(&t));
            }
        }
    }
    let mut rng = Rng(0x0f1e_2d3c_4b5a_6978);
    for i in 0..6000 {
        let start = rng.point();
        let disp = match i % 4 {
            0 => rng.disp(8.0),
            1 => rng.disp(80.0),
            2 => rng.disp(600.0),
            // Vertex-directed: exactly at a chart corner.
            _ => Vec2::new(64.0 - start.u, -start.v),
        };
        d.bulk(&travel_row(&travel(Topology::Cube, start, disp)));
    }
    d.seal("travel");

    // --- chart images.
    for face in Face::ALL {
        let mut images = Vec::new();
        chart_images(Topology::Cube, face, 2, &mut images);
        d.row(&format!("chart_images {} count {}", face.index(), images.len()));
        for img in &images {
            d.bulk(&format!(
                "{} {}{:?} -> {} origin={} map={:?}",
                face.index(),
                img.path.len,
                img.path.steps(),
                img.target_face.index(),
                v(img.origin),
                img.map.m
            ));
        }
    }
    d.seal("chart_images");

    // --- unfold over a fixed pair set, near and far.
    let mut rng = Rng(0xabcd_1234_5678_9f01);
    let mut hits = 0u32;
    for i in 0..20000 {
        let a = rng.point();
        let b = if i % 2 == 0 {
            travel(Topology::Cube, a, Vec2::from_screen_angle(rng.next_f64() * std::f64::consts::TAU) * (rng.next_f64() * 14.0)).end
        } else {
            rng.point()
        };
        let row = match unfold(Topology::Cube, a, b, 12.0) {
            Some(u) => {
                hits += 1;
                format!("unfold {} {} -> {}", sp(&a), sp(&b), unfolded_row(&u))
            }
            None => format!("unfold {} {} -> none", sp(&a), sp(&b)),
        };
        d.bulk(&row);
    }
    d.row(&format!("unfold in-range pairs {hits}"));
    // A handful in full, chosen so each chart and each seam kind appears.
    for &(fa, ua, va, fb, ub, vb) in &[
        (Face::Front, 63.5, 20.5, Face::Right, 0.5, 20.5),
        (Face::Right, 10.5, 0.5, Face::Top, 63.5, 53.5),
        (Face::Back, 10.5, 0.5, Face::Top, 53.5, 0.5),
        (Face::Front, 60.0, 3.0, Face::Right, 3.0, 4.0),
        (Face::Left, 0.5, 0.5, Face::Front, 63.5, 0.5),
        (Face::Top, 32.5, 32.5, Face::Top, 40.5, 20.5),
    ] {
        let a = SurfacePoint::new(fa, ua, va);
        let b = SurfacePoint::new(fb, ub, vb);
        let row = match unfold(Topology::Cube, a, b, 12.0) {
            Some(u) => unfolded_row(&u),
            None => "none".to_string(),
        };
        d.row(&format!("unfold {} {} -> {row}", sp(&a), sp(&b)));
    }
    d.seal("unfold");

    // --- unfold_pixels at every named anchor and radius, every pixel in full in the digest.
    let mut out = Vec::new();
    for &(face, u, w) in ANCHORS {
        for radius in [0.0f64, 1.5, 6.0, 9.0, 12.0, 32.0] {
            let anchor = SurfacePoint::new(face, u, w).canonicalize(Topology::Cube);
            unfold_pixels(Topology::Cube, anchor, radius, &mut out);
            d.row(&format!("unfold_pixels {} r={} -> {} pixels", sp(&anchor), f(radius), out.len()));
            for p in &out {
                d.bulk(&pixel_row(p));
            }
        }
    }
    d.seal("unfold_pixels");

    // --- the field graph: cells, neighbours, degrees, downhill, every edge.
    let g = FieldGraph::new(Topology::Cube, Scale::ONE);
    d.row(&format!("cells {CUBE_CELL_COUNT}"));
    d.row(&format!("edges {}", g.edges().len()));
    let seam_edges = g.edges().iter().filter(|(a, b)| a.face(Topology::Cube, Scale::ONE) != b.face(Topology::Cube, Scale::ONE)).count();
    d.row(&format!("seam edges {seam_edges}"));
    let mut degrees = [0usize; 5];
    for cell in CellId::all(Topology::Cube, Scale::ONE) {
        degrees[g.degree(cell)] += 1;
    }
    d.row(&format!("degree histogram {degrees:?}"));
    let downhill_none = CellId::all(Topology::Cube, Scale::ONE).filter(|c| g.downhill(*c).is_none()).count();
    d.row(&format!("downhill none {downhill_none}"));
    for cell in CellId::all(Topology::Cube, Scale::ONE) {
        let c = cell.center(Topology::Cube, Scale::ONE);
        d.bulk(&format!(
            "{} face={} cx={} cy={} centre={} cell_of={} ns={:?} down={:?}",
            cell.0,
            cell.face(Topology::Cube, Scale::ONE).index(),
            cell.cx(Topology::Cube, Scale::ONE),
            cell.cy(Topology::Cube, Scale::ONE),
            sp(&c),
            cell_of(Topology::Cube, Scale::ONE, &c).0,
            g.neighbors(cell).map(|n| n.map(|c| c.0)),
            g.downhill(cell).map(|c| c.0)
        ));
    }
    for &(a, b) in g.edges() {
        d.bulk(&format!("edge {} {}", a.0, b.0));
    }
    d.seal("field_graph");

    // --- deposit and diffuse: the numbers a footprint actually writes.
    let mut field = ScalarField::zeros(Topology::Cube, Scale::ONE);
    for (i, &(face, u, w)) in ANCHORS.iter().enumerate() {
        let centre = SurfacePoint::new(face, u, w).canonicalize(Topology::Cube);
        let radius = [1.0f64, 4.0, 9.0][i % 3];
        let touched = deposit(Topology::Cube, Scale::ONE, &mut field, centre, radius, 1.0 + i as f64);
        d.row(&format!("deposit {} r={} -> {touched} cells", sp(&centre), f(radius)));
    }
    d.row(&format!("field total {}", f(field.total())));
    let mut scratch = ScalarField::zeros(Topology::Cube, Scale::ONE);
    let substeps = diffuse(&mut field, &mut scratch, &g, 0.9);
    d.row(&format!("diffuse substeps {substeps} total {}", f(field.total())));
    for cell in CellId::all(Topology::Cube, Scale::ONE) {
        d.bulk(&format!("{} {}", cell.0, f(field.get(cell))));
    }
    d.seal("deposit_diffuse");

    d.text
}

#[test]
fn cube_outputs_are_unchanged_by_value() {
    let got = dump();
    if std::env::var_os("CUBARIUM_UPDATE_GOLDEN").is_some() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/cube_by_value.txt");
        std::fs::write(path, &got).expect("write golden");
        eprintln!("wrote {path}");
        return;
    }
    if got != GOLDEN {
        let a: Vec<&str> = GOLDEN.lines().collect();
        let b: Vec<&str> = got.lines().collect();
        for (i, (x, y)) in a.iter().zip(&b).enumerate() {
            assert_eq!(x, y, "cube output changed at golden line {}", i + 1);
        }
        panic!("golden has {} lines, dump has {}", a.len(), b.len());
    }
}
