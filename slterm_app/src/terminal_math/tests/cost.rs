use super::*;

// 扫描与冷编译成本量尺（默认忽略）。

/// 每帧扫描与冷编译的量尺，不是断言。跑法：
/// `cargo test -p slterm --test lib_tests terminal_math::tests::cost::measure_scan -- --ignored --nocapture`
#[test]
#[ignore = "timing probe, run by hand"]
fn measure_scan_and_compile_cost() {
    use std::time::Instant;

    // 50×200 的视口，塞满带 `$`、`[`、`(` 的正文：每个候选都触发一次搜索
    // 却没有一个能成公式，是扫描器最贵的形状。
    let prose = "price is $5 and $HOME/bin (see [1]) costs 3 (x) dollars $ USD [a, b] ";
    let row: String = prose
        .repeat(200 / prose.chars().count() + 1)
        .chars()
        .take(200)
        .collect();
    let rows: Vec<&str> = (0..50).map(|_| row.as_str()).collect();
    let grid = TextGrid::from_rows(&rows);
    let iterations = 200;
    let start = Instant::now();
    for _ in 0..iterations {
        let scan = scan_grid_result(&grid);
        assert!(scan.overlays.is_empty());
    }
    let per_scan = start.elapsed() / iterations;
    println!("scan 50x200 prose (worst case, 0 formulas): {per_scan:?} per frame");

    // 同样尺寸，塞满能命中的行内公式：每帧的公式上限是 MAX_VISIBLE_FORMULAS。
    let formula_row = "the energy $E = mc^2$ and $\\int_0^1 x\\,dx = \\frac{1}{2}$ hold; ";
    let row: String = formula_row
        .repeat(200 / formula_row.chars().count() + 1)
        .chars()
        .take(200)
        .collect();
    let rows: Vec<&str> = (0..50).map(|_| row.as_str()).collect();
    let grid = TextGrid::from_rows(&rows);
    let start = Instant::now();
    for _ in 0..iterations {
        let scan = scan_grid_result(&grid);
        assert_eq!(scan.overlays.len(), MAX_VISIBLE_FORMULAS);
    }
    let per_scan = start.elapsed() / iterations;
    println!(
        "scan 50x200 inline formulas (capped at {MAX_VISIBLE_FORMULAS}): {per_scan:?} per frame"
    );

    let screenshot: [&str; 7] = [
        r"e^{i\pi}+1=0",
        r"\int_{-\infty}^{+\infty} e^{-x^2},dx=\sqrt{\pi}",
        "\\widehat{f}(\\xi)\n\n\\int_{-\\infty}^{+\\infty}\nf(x)e^{-2\\pi i x\\xi},dx",
        "\\operatorname{Attention}(Q,K,V)\n\n\\operatorname{softmax}\\left(\n\\frac{QK^{\\mathsf T}}{\\sqrt{d_k}}\n\\right)V",
        "\\phi(x)=\n\\begin{cases}\nx, & x\\ge 0,\\\n\\alpha(e^x-1), & x<0.\n\\end{cases}",
        "A=\n\\begin{pmatrix}\na_{11} & a_{12} & \\cdots & a_{1n}\\\na_{21} & a_{22} & \\cdots & a_{2n}\\\n\\vdots & \\vdots & \\ddots & \\vdots\\\na_{m1} & a_{m2} & \\cdots & a_{mn}\n\\end{pmatrix}",
        "\\begin{aligned}\n(a+b)^3\n&=(a+b)(a+b)^2\\\n&=(a+b)(a^2+2ab+b^2)\\\n&=a^3+3a^2b+3ab^2+b^3\n\\end{aligned}",
    ];
    for source in screenshot {
        let start = Instant::now();
        for _ in 0..iterations {
            compile_formula(source, true, 18.0, 1.0, DEFAULT_LIMITS).expect("compiles");
        }
        let per_compile = start.elapsed() / iterations;
        let head: String = source
            .chars()
            .take(28)
            .map(|c| if c == '\n' { ' ' } else { c })
            .collect();
        println!("compile {head:<30} {per_compile:?} cold (cache hit is a map lookup)");
    }
}
