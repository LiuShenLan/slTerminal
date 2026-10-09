//! 内建字形几何：框线、块元素与 Powerline 分隔符。
//!
//! 从 slterm_app 的 builtin_font（像素栅格化）移植而来，改为输出纯几何
//! 指令（矩形 + 凸多边形），由前端用自己的图元 API 填充。放进渲染合同的
//! 理由与 `RenderSnapshot` 相同：这些字符必须精确盖满单元格、相邻单元格
//! 必须无缝拼接，字体（尤其 CJK 字体）保证不了这一点；几何是唯一权威，
//! 且任何渲染后端都免费继承同一套字形。
//!
//! 坐标系：单元格局部、逻辑像素、y 向下。内部先换算到设备像素做整数
//! 吸附，直线和圆角共享笔画边界规则，出口再除回 scale——这是笔画清晰、
//! 跨行列连续的关键。与旧实现的另外两点已知差异：
//! - Powerline 三角/箭头在窄单元格下不再回退字体，而是在右缘截平
//!   （几何裁剪天然优雅，无需 None 逃逸路径）；
//! - ░▒▓ 以 alpha 表达浓度（64/128/192 ÷ 255，与旧像素灰度一致）。

use std::f32::consts::{FRAC_PI_2, PI};

/// 该字符是否由内建几何绘制。快照据此把单元格路由到
/// [`super::RenderSnapshot::box_glyphs`] 而不是文本段。
pub fn is_builtin(c: char) -> bool {
    matches!(
        c,
        // 框线与块元素。
        '\u{2500}'..='\u{259f}'
        // Legacy Computing 六分块与上部块。
        | '\u{1fb00}'..='\u{1fb3b}'
        | '\u{1fb82}'..='\u{1fb8b}'
        // Powerline 三角/箭头与圆头（e0b5 不在内建集内）。
        | '\u{e0b0}'..='\u{e0b4}'
        | '\u{e0b6}'
    )
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// 单个填充图元，单元格局部逻辑像素坐标。
#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    /// 轴对齐实心矩形；alpha 仅 ░▒▓ 使用（浓度档），其余为 1。
    Rect { rect: Rect, alpha: f32 },
    /// 凸多边形（对角线、Powerline 笔画、圆弧分段）。顶点保证凸序，
    /// 消费方可安全用首点三角扇填充。
    Poly { points: Vec<[f32; 2]> },
}

/// 生成 `c` 的填充图元。`width`/`height` 是该字形占据的逻辑像素盒
/// （宽字符由调用方先乘 2），`scale` 是设备缩放。非内建字符返回 `None`。
pub fn primitives(c: char, width: f32, height: f32, scale: f32) -> Option<Vec<Primitive>> {
    if !is_builtin(c) {
        return None;
    }
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let mut geom = Geom::new(
        (width * scale).round().max(1.0),
        (height * scale).round().max(1.0),
    );
    draw(c, &mut geom);
    Some(geom.finish(scale))
}

/// 未裁剪的笔画边界。直线和圆角必须对同一半像素作相同的取整，
/// 否则单元格尺寸与笔画宽度的奇偶组合会让接头错开一个设备像素。
fn stroke_bounds(center: f32, stroke: f32) -> (f32, f32) {
    (
        (center - stroke / 2.0).round(),
        (center + stroke / 2.0).round(),
    )
}

/// 设备像素坐标下的图元累加器：像素吸附与画布边缘裁剪后输出几何。
struct Geom {
    w: f32,
    h: f32,
    /// 细笔画宽：单元格宽的 1/8（四舍五入），最小 1 设备像素。
    stroke: f32,
    out: Vec<Primitive>,
}

impl Geom {
    fn new(w: f32, h: f32) -> Self {
        let stroke = (w / 8.0).round().max(1.0);
        Self {
            w,
            h,
            stroke,
            out: Vec::new(),
        }
    }

    fn x_center(&self) -> f32 {
        self.w / 2.0
    }

    fn y_center(&self) -> f32 {
        self.h / 2.0
    }

    /// 横线在 `y` 处、笔画宽 `stroke` 的上下界（整数吸附后裁剪）。
    fn h_line_bounds(&self, y: f32, stroke: f32) -> (f32, f32) {
        let (top, bottom) = stroke_bounds(y, stroke);
        (top.max(0.0), bottom.min(self.h))
    }

    fn v_line_bounds(&self, x: f32, stroke: f32) -> (f32, f32) {
        let (left, right) = stroke_bounds(x, stroke);
        (left.max(0.0), right.min(self.w))
    }

    /// 实心矩形，裁剪到单元格（旧 Canvas 在画布边缘截断的对应物）。
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.rect_alpha(x, y, w, h, 1.0);
    }

    fn rect_alpha(&mut self, x: f32, y: f32, w: f32, h: f32, alpha: f32) {
        let x0 = x.max(0.0);
        let y0 = y.max(0.0);
        let x1 = (x + w).min(self.w);
        let y1 = (y + h).min(self.h);
        if x1 - x0 > 0.0 && y1 - y0 > 0.0 {
            self.out.push(Primitive::Rect {
                rect: Rect {
                    x: x0,
                    y: y0,
                    w: x1 - x0,
                    h: y1 - y0,
                },
                alpha,
            });
        }
    }

    /// 从 (`x`, `y`) 起、长 `size` 的横线；`size` 为 0 或笔画为 0 时无操作。
    fn h_line(&mut self, x: f32, y: f32, size: f32, stroke: f32) {
        let (top, bottom) = self.h_line_bounds(y, stroke);
        self.rect(x, top, size, bottom - top);
    }

    fn v_line(&mut self, x: f32, y: f32, size: f32, stroke: f32) {
        let (left, right) = self.v_line_bounds(x, stroke);
        self.rect(left, y, right - left, size);
    }

    /// 凸多边形，顶点不做单元格裁剪：对角线依赖少量出格实现跨行连续。
    fn poly(&mut self, points: Vec<[f32; 2]>) {
        self.out.push(Primitive::Poly { points });
    }

    /// 把一个凸多边形裁到当前 cell。圆角框线是把扩展的圆角矩形裁回
    /// cell 后得到的可见弧段；如果让这些顶点越过 cell，GPUI 会把圆角
    /// 的外半径泄漏到相邻字符里。对角线和 Powerline 仍使用 [`Self::poly`]
    /// 保留跨 cell 的连接语义。
    fn poly_clipped(&mut self, mut points: Vec<[f32; 2]>) {
        for (axis, bound, keep_greater) in [
            (0, 0.0, true),
            (0, self.w, false),
            (1, 0.0, true),
            (1, self.h, false),
        ] {
            points = clip_polygon_axis(points, axis, bound, keep_greater);
            if points.len() < 3 {
                return;
            }
        }
        self.out.push(Primitive::Poly { points });
    }

    /// 设备像素 → 逻辑像素。
    fn finish(self, scale: f32) -> Vec<Primitive> {
        let inv = 1.0 / scale;
        self.out
            .into_iter()
            .map(|p| match p {
                Primitive::Rect { rect, alpha } => Primitive::Rect {
                    rect: Rect {
                        x: rect.x * inv,
                        y: rect.y * inv,
                        w: rect.w * inv,
                        h: rect.h * inv,
                    },
                    alpha,
                },
                Primitive::Poly { points } => Primitive::Poly {
                    points: points
                        .into_iter()
                        .map(|[x, y]| [x * inv, y * inv])
                        .collect(),
                },
            })
            .collect()
    }
}

/// Sutherland–Hodgman clipping for the four axis-aligned cell edges.
fn clip_polygon_axis(
    points: Vec<[f32; 2]>,
    axis: usize,
    bound: f32,
    keep_greater: bool,
) -> Vec<[f32; 2]> {
    let Some(mut previous) = points.last().copied() else {
        return Vec::new();
    };
    let mut previous_inside = if keep_greater {
        previous[axis] >= bound
    } else {
        previous[axis] <= bound
    };
    let mut clipped = Vec::with_capacity(points.len() + 1);
    for current in points {
        let current_inside = if keep_greater {
            current[axis] >= bound
        } else {
            current[axis] <= bound
        };
        if current_inside != previous_inside {
            let delta = current[axis] - previous[axis];
            if delta.abs() > f32::EPSILON {
                let t = (bound - previous[axis]) / delta;
                clipped.push([
                    previous[0] + (current[0] - previous[0]) * t,
                    previous[1] + (current[1] - previous[1]) * t,
                ]);
            }
        }
        if current_inside {
            clipped.push(current);
        }
        previous = current;
        previous_inside = current_inside;
    }
    clipped
}

fn draw(c: char, g: &mut Geom) {
    let stroke = g.stroke;
    let heavy = stroke * 2.0;
    let (w, h) = (g.w, g.h);
    match c {
        // 对角线 '╱','╲','╳'：中心线贯穿两个对角，竖直方向厚 stroke，
        // 端点上下各出格半个笔画以在相邻行间连续（旧实现用放大画布）。
        '\u{2571}'..='\u{2573}' => {
            let half = stroke / 2.0;
            if c != '\u{2572}' {
                // '╱'：左下 → 右上。
                g.poly(vec![
                    [0.0, h - half],
                    [0.0, h + half],
                    [w, half],
                    [w, -half],
                ]);
            }
            if c != '\u{2571}' {
                // '╲'：左上 → 右下。
                g.poly(vec![
                    [0.0, -half],
                    [0.0, half],
                    [w, h + half],
                    [w, h - half],
                ]);
            }
        }
        // 横向虚线 '┄','┅','┈','┉','╌','╍'。
        '\u{2504}' | '\u{2505}' | '\u{2508}' | '\u{2509}' | '\u{254c}' | '\u{254d}' => {
            let (num_gaps, stroke) = match c {
                '\u{2504}' => (2, stroke),
                '\u{2505}' => (2, heavy),
                '\u{2508}' => (3, stroke),
                '\u{2509}' => (3, heavy),
                '\u{254c}' => (1, stroke),
                '\u{254d}' => (1, heavy),
                _ => unreachable!(),
            };
            let gap_len = (w / 8.0).floor().max(1.0);
            let dash_len = (((w - gap_len * num_gaps as f32).max(0.0)) / (num_gaps + 1) as f32)
                .floor()
                .max(1.0);
            let y = g.y_center();
            for gap in 0..=num_gaps {
                let x = (gap as f32 * (dash_len + gap_len)).min(w);
                g.h_line(x, y, dash_len, stroke);
            }
        }
        // 纵向虚线 '┆','┇','┊','┋','╎','╏'。
        '\u{2506}' | '\u{2507}' | '\u{250a}' | '\u{250b}' | '\u{254e}' | '\u{254f}' => {
            let (num_gaps, stroke) = match c {
                '\u{2506}' => (2, stroke),
                '\u{2507}' => (2, heavy),
                '\u{250a}' => (3, stroke),
                '\u{250b}' => (3, heavy),
                '\u{254e}' => (1, stroke),
                '\u{254f}' => (1, heavy),
                _ => unreachable!(),
            };
            let gap_len = (h / 8.0).floor().max(1.0);
            let dash_len = (((h - gap_len * num_gaps as f32).max(0.0)) / (num_gaps + 1) as f32)
                .floor()
                .max(1.0);
            let x = g.x_center();
            for gap in 0..=num_gaps {
                let y = (gap as f32 * (dash_len + gap_len)).min(h);
                g.v_line(x, y, dash_len, stroke);
            }
        }
        // 单/重线箱形组件：横竖直线、角、T 形、十字与轻重混合过渡。
        // 分解为左右上下四条臂，各自笔画为 0/细/重（表与旧实现逐字一致）。
        '\u{2500}'..='\u{2503}' | '\u{250c}'..='\u{254b}' | '\u{2574}'..='\u{257f}' => {
            // 左臂。
            let stroke_h1 = match c {
                '\u{2500}' | '\u{2510}' | '\u{2512}' | '\u{2518}' | '\u{251a}' | '\u{2524}'
                | '\u{2526}' | '\u{2527}' | '\u{2528}' | '\u{252c}' | '\u{252e}' | '\u{2530}'
                | '\u{2532}' | '\u{2534}' | '\u{2536}' | '\u{2538}' | '\u{253a}' | '\u{253c}'
                | '\u{253e}' | '\u{2540}' | '\u{2541}' | '\u{2542}' | '\u{2544}' | '\u{2546}'
                | '\u{254a}' | '\u{2574}' | '\u{257c}' => stroke,
                '\u{2501}' | '\u{2511}' | '\u{2513}' | '\u{2519}' | '\u{251b}' | '\u{2525}'
                | '\u{2529}' | '\u{252a}' | '\u{252b}' | '\u{252d}' | '\u{252f}' | '\u{2531}'
                | '\u{2533}' | '\u{2535}' | '\u{2537}' | '\u{2539}' | '\u{253b}' | '\u{253d}'
                | '\u{253f}' | '\u{2543}' | '\u{2545}' | '\u{2547}' | '\u{2548}' | '\u{2549}'
                | '\u{254b}' | '\u{2578}' | '\u{257e}' => heavy,
                _ => 0.0,
            };
            // 右臂。
            let stroke_h2 = match c {
                '\u{2500}' | '\u{250c}' | '\u{250e}' | '\u{2514}' | '\u{2516}' | '\u{251c}'
                | '\u{251e}' | '\u{251f}' | '\u{2520}' | '\u{252c}' | '\u{252d}' | '\u{2530}'
                | '\u{2531}' | '\u{2534}' | '\u{2535}' | '\u{2538}' | '\u{2539}' | '\u{253c}'
                | '\u{253d}' | '\u{2540}' | '\u{2541}' | '\u{2542}' | '\u{2543}' | '\u{2545}'
                | '\u{2549}' | '\u{2576}' | '\u{257e}' => stroke,
                '\u{2501}' | '\u{250d}' | '\u{250f}' | '\u{2515}' | '\u{2517}' | '\u{251d}'
                | '\u{2521}' | '\u{2522}' | '\u{2523}' | '\u{252e}' | '\u{252f}' | '\u{2532}'
                | '\u{2533}' | '\u{2536}' | '\u{2537}' | '\u{253a}' | '\u{253b}' | '\u{253e}'
                | '\u{253f}' | '\u{2544}' | '\u{2546}' | '\u{2547}' | '\u{2548}' | '\u{254a}'
                | '\u{254b}' | '\u{257a}' | '\u{257c}' => heavy,
                _ => 0.0,
            };
            // 上臂。
            let stroke_v1 = match c {
                '\u{2502}' | '\u{2514}' | '\u{2515}' | '\u{2518}' | '\u{2519}' | '\u{251c}'
                | '\u{251d}' | '\u{251f}' | '\u{2522}' | '\u{2524}' | '\u{2525}' | '\u{2527}'
                | '\u{252a}' | '\u{2534}' | '\u{2535}' | '\u{2536}' | '\u{2537}' | '\u{253c}'
                | '\u{253d}' | '\u{253e}' | '\u{253f}' | '\u{2541}' | '\u{2545}' | '\u{2546}'
                | '\u{2548}' | '\u{2575}' | '\u{257d}' => stroke,
                '\u{2503}' | '\u{2516}' | '\u{2517}' | '\u{251a}' | '\u{251b}' | '\u{251e}'
                | '\u{2520}' | '\u{2521}' | '\u{2523}' | '\u{2526}' | '\u{2528}' | '\u{2529}'
                | '\u{252b}' | '\u{2538}' | '\u{2539}' | '\u{253a}' | '\u{253b}' | '\u{2540}'
                | '\u{2542}' | '\u{2543}' | '\u{2544}' | '\u{2547}' | '\u{2549}' | '\u{254a}'
                | '\u{254b}' | '\u{2579}' | '\u{257f}' => heavy,
                _ => 0.0,
            };
            // 下臂。
            let stroke_v2 = match c {
                '\u{2502}' | '\u{250c}' | '\u{250d}' | '\u{2510}' | '\u{2511}' | '\u{251c}'
                | '\u{251d}' | '\u{251e}' | '\u{2521}' | '\u{2524}' | '\u{2525}' | '\u{2526}'
                | '\u{2529}' | '\u{252c}' | '\u{252d}' | '\u{252e}' | '\u{252f}' | '\u{253c}'
                | '\u{253d}' | '\u{253e}' | '\u{253f}' | '\u{2540}' | '\u{2543}' | '\u{2544}'
                | '\u{2547}' | '\u{2577}' | '\u{257f}' => stroke,
                '\u{2503}' | '\u{250e}' | '\u{250f}' | '\u{2512}' | '\u{2513}' | '\u{251f}'
                | '\u{2520}' | '\u{2522}' | '\u{2523}' | '\u{2527}' | '\u{2528}' | '\u{252a}'
                | '\u{252b}' | '\u{2530}' | '\u{2531}' | '\u{2532}' | '\u{2533}' | '\u{2541}'
                | '\u{2542}' | '\u{2545}' | '\u{2546}' | '\u{2548}' | '\u{2549}' | '\u{254a}'
                | '\u{254b}' | '\u{257b}' | '\u{257d}' => heavy,
                _ => 0.0,
            };

            let x_v = g.x_center();
            let y_h = g.y_center();

            let v_top = g.v_line_bounds(x_v, stroke_v1);
            let v_bot = g.v_line_bounds(x_v, stroke_v2);
            let h_left = g.h_line_bounds(y_h, stroke_h1);
            let h_right = g.h_line_bounds(y_h, stroke_h2);

            // 臂长延伸到对向臂的笔画边界，保证交点处无缺口。
            let size_h1 = v_top.1.max(v_bot.1);
            let x_h = v_top.0.min(v_bot.0);
            let size_h2 = w - x_h;
            let size_v1 = h_left.1.max(h_right.1);
            let y_v = h_left.0.min(h_right.0);
            let size_v2 = h - y_v;

            g.h_line(0.0, y_h, size_h1, stroke_h1);
            g.h_line(x_h, y_h, size_h2, stroke_h2);
            g.v_line(x_v, 0.0, size_v1, stroke_v1);
            g.v_line(x_v, y_v, size_v2, stroke_v2);
        }
        // 单/双线组合 '═','║','╒'..'╬'：双线是围绕单线边界外扩 1 设备像素
        // 的两条平行细线；各分支的截止点表与旧实现逐字一致。
        '\u{2550}'..='\u{256c}' => {
            let v_lines = match c {
                '\u{2552}' | '\u{2555}' | '\u{2558}' | '\u{255b}' | '\u{255e}' | '\u{2561}'
                | '\u{2564}' | '\u{2567}' | '\u{256a}' => (g.x_center(), g.x_center()),
                _ => {
                    let bounds = g.v_line_bounds(g.x_center(), stroke);
                    let left = (bounds.0 as i32 - 1).max(0) as f32;
                    let right = (bounds.1 as i32 + 1).min(w as i32) as f32;
                    (left, right)
                }
            };
            let h_lines = match c {
                '\u{2553}' | '\u{2556}' | '\u{2559}' | '\u{255c}' | '\u{255f}' | '\u{2562}'
                | '\u{2565}' | '\u{2568}' | '\u{256b}' => (g.y_center(), g.y_center()),
                _ => {
                    let bounds = g.h_line_bounds(g.y_center(), stroke);
                    let top = (bounds.0 as i32 - 1).max(0) as f32;
                    let bottom = (bounds.1 as i32 + 1).min(h as i32) as f32;
                    (top, bottom)
                }
            };

            let v_left_bounds = g.v_line_bounds(v_lines.0, stroke);
            let v_right_bounds = g.v_line_bounds(v_lines.1, stroke);
            let h_top_bounds = g.h_line_bounds(h_lines.0, stroke);
            let h_bot_bounds = g.h_line_bounds(h_lines.1, stroke);

            // 左半横线（上、下两条）。
            let (top_left_size, bot_left_size) = match c {
                '\u{2550}' | '\u{256b}' => (g.x_center(), g.x_center()),
                '\u{2555}'..='\u{2557}' => (v_right_bounds.1, v_left_bounds.1),
                '\u{255b}'..='\u{255d}' => (v_left_bounds.1, v_right_bounds.1),
                '\u{2561}'..='\u{2563}' | '\u{256a}' | '\u{256c}' => {
                    (v_left_bounds.1, v_left_bounds.1)
                }
                '\u{2564}'..='\u{2568}' => (g.x_center(), v_left_bounds.1),
                '\u{2569}' => (v_left_bounds.1, g.x_center()),
                _ => (0.0, 0.0),
            };
            // 右半横线。
            let (top_right_x, bot_right_x, right_size) = match c {
                '\u{2550}' | '\u{2565}' | '\u{256b}' => (g.x_center(), g.x_center(), w),
                '\u{2552}'..='\u{2554}' | '\u{2568}' => (v_left_bounds.0, v_right_bounds.0, w),
                '\u{2558}'..='\u{255a}' => (v_right_bounds.0, v_left_bounds.0, w),
                '\u{255e}'..='\u{2560}' | '\u{256a}' | '\u{256c}' => {
                    (v_right_bounds.0, v_right_bounds.0, w)
                }
                '\u{2564}' | '\u{2566}' => (g.x_center(), v_right_bounds.0, w),
                '\u{2567}' | '\u{2569}' => (v_right_bounds.0, g.x_center(), w),
                _ => (0.0, 0.0, 0.0),
            };
            // 上半竖线（左、右两条）。
            let (left_top_size, right_top_size) = match c {
                '\u{2551}' | '\u{256a}' => (g.y_center(), g.y_center()),
                '\u{2558}'..='\u{255c}' | '\u{2568}' => (h_bot_bounds.1, h_top_bounds.1),
                '\u{255d}' => (h_top_bounds.1, h_bot_bounds.1),
                '\u{255e}'..='\u{2560}' => (g.y_center(), h_top_bounds.1),
                '\u{2561}'..='\u{2563}' => (h_top_bounds.1, g.y_center()),
                '\u{2567}' | '\u{2569}' | '\u{256b}' | '\u{256c}' => {
                    (h_top_bounds.1, h_top_bounds.1)
                }
                _ => (0.0, 0.0),
            };
            // 下半竖线。
            let (left_bot_y, right_bot_y, bottom_size) = match c {
                '\u{2551}' | '\u{256a}' => (g.y_center(), g.y_center(), h),
                '\u{2552}'..='\u{2554}' => (h_top_bounds.0, h_bot_bounds.0, h),
                '\u{2555}'..='\u{2557}' => (h_bot_bounds.0, h_top_bounds.0, h),
                '\u{255e}'..='\u{2560}' => (g.y_center(), h_bot_bounds.0, h),
                '\u{2561}'..='\u{2563}' => (h_bot_bounds.0, g.y_center(), h),
                '\u{2564}'..='\u{2566}' | '\u{256b}' | '\u{256c}' => {
                    (h_bot_bounds.0, h_bot_bounds.0, h)
                }
                _ => (0.0, 0.0, 0.0),
            };

            g.h_line(0.0, h_lines.0, top_left_size, stroke);
            g.h_line(0.0, h_lines.1, bot_left_size, stroke);
            g.h_line(top_right_x, h_lines.0, right_size, stroke);
            g.h_line(bot_right_x, h_lines.1, right_size, stroke);
            g.v_line(v_lines.0, 0.0, left_top_size, stroke);
            g.v_line(v_lines.1, 0.0, right_top_size, stroke);
            g.v_line(v_lines.0, left_bot_y, bottom_size, stroke);
            g.v_line(v_lines.1, right_bot_y, bottom_size, stroke);
        }
        // 圆角 '╭','╮','╯','╰'：按 WT BuiltinGlyphs 的扩展 rounded-rect
        // 语义生成两段直臂和一段四分之一圆环。rounded-rect 故意延伸到
        // cell 外，再裁回当前 cell；这样相邻的 `╭─╮`/`╰─╯` 连接点与
        // 普通框线共享同一条像素边界，不会在圆角处留下毛刺或多出一列墨迹。
        // 笔画宽继续沿用 Slterm 的 `/8` 合同，只有圆角的位置和裁剪规则
        // 对齐 WT。
        '\u{256d}'..='\u{2570}' => {
            // Direct2D 的原实现先把路径坐标按半笔画修正，再对绝对坐标
            // round；在设备像素域复刻同一计算，避免奇数 cell 宽度下圆角
            // 比相邻直线偏半个像素。
            let t = stroke;
            let half = t / 2.0;
            let snap_beg = |base: f32| stroke_bounds(base, t).0 + half;
            let snap_end = |base: f32| stroke_bounds(base, t).1 - half;
            let (left, top, right, bottom, start_angle, end_angle) = match c {
                '\u{256d}' => (
                    snap_beg(w * 0.5),
                    snap_beg(h * 0.5),
                    snap_end(w * 1.5),
                    snap_end(h * 1.5),
                    PI,
                    PI + FRAC_PI_2,
                ),
                '\u{256e}' => (
                    snap_beg(w * -0.5),
                    snap_beg(h * 0.5),
                    snap_end(w * 0.5),
                    snap_end(h * 1.5),
                    -FRAC_PI_2,
                    0.0,
                ),
                '\u{256f}' => (
                    snap_beg(w * -0.5),
                    snap_beg(h * -0.5),
                    snap_end(w * 0.5),
                    snap_end(h * 0.5),
                    0.0,
                    FRAC_PI_2,
                ),
                '\u{2570}' => (
                    snap_beg(w * 0.5),
                    snap_beg(h * -0.5),
                    snap_end(w * 1.5),
                    snap_end(h * 0.5),
                    FRAC_PI_2,
                    PI,
                ),
                _ => unreachable!(),
            };
            let radius = (t * 5.0).min(w.min(h) * 0.5);
            let (cx, cy, vertical_start, vertical_len, horizontal_start, horizontal_len) = match c {
                '\u{256d}' => (
                    left + radius,
                    top + radius,
                    left,
                    h - (top + radius),
                    left + radius,
                    w - (left + radius),
                ),
                '\u{256e}' => (
                    right - radius,
                    top + radius,
                    right,
                    h - (top + radius),
                    0.0,
                    right - radius,
                ),
                '\u{256f}' => (
                    right - radius,
                    bottom - radius,
                    right,
                    bottom - radius,
                    0.0,
                    right - radius,
                ),
                '\u{2570}' => (
                    left + radius,
                    bottom - radius,
                    left,
                    bottom - radius,
                    left + radius,
                    w - (left + radius),
                ),
                _ => unreachable!(),
            };

            match c {
                '\u{256d}' | '\u{256e}' => {
                    g.v_line(vertical_start, top + radius, vertical_len, t);
                    g.h_line(horizontal_start, top, horizontal_len, t);
                }
                '\u{256f}' | '\u{2570}' => {
                    g.v_line(vertical_start, 0.0, vertical_len, t);
                    g.h_line(horizontal_start, bottom, horizontal_len, t);
                }
                _ => unreachable!(),
            }

            // A stroked rounded rectangle has a centerline radius `radius`; its
            // filled annulus therefore uses +/- half the stroke width. Clip
            // every convex segment to the cell like WT's aliased clip.
            let outer = radius + half;
            let inner = (radius - half).max(0.0);
            const SEGMENTS: usize = 8;
            for i in 0..SEGMENTS {
                let a0 = start_angle + (end_angle - start_angle) * i as f32 / SEGMENTS as f32;
                let a1 = start_angle + (end_angle - start_angle) * (i + 1) as f32 / SEGMENTS as f32;
                let at = |angle: f32, r: f32| [cx + r * angle.cos(), cy + r * angle.sin()];
                g.poly_clipped(vec![
                    at(a0, outer),
                    at(a1, outer),
                    at(a1, inner),
                    at(a0, inner),
                ]);
            }
        }
        // 局部块：上/下 n/8、左/右 n/8（含 Legacy Computing 上部块）。
        '\u{2580}'..='\u{2587}'
        | '\u{2589}'..='\u{2590}'
        | '\u{2594}'
        | '\u{2595}'
        | '\u{1fb82}'..='\u{1fb8b}' => {
            let mut rect_width = match c {
                '\u{2589}' | '\u{1fb8b}' => w * 7.0 / 8.0,
                '\u{258a}' | '\u{1fb8a}' => w * 6.0 / 8.0,
                '\u{258b}' | '\u{1fb89}' => w * 5.0 / 8.0,
                '\u{258c}' => w * 4.0 / 8.0,
                '\u{258d}' | '\u{1fb88}' => w * 3.0 / 8.0,
                '\u{258e}' | '\u{1fb87}' => w * 2.0 / 8.0,
                '\u{258f}' => w * 1.0 / 8.0,
                '\u{2590}' => w * 4.0 / 8.0,
                '\u{2595}' => w * 1.0 / 8.0,
                _ => w,
            };
            let (mut rect_height, y) = match c {
                '\u{2580}' => (h * 4.0 / 8.0, h),
                '\u{2581}' => (h * 1.0 / 8.0, h * 1.0 / 8.0),
                '\u{2582}' => (h * 2.0 / 8.0, h * 2.0 / 8.0),
                '\u{2583}' => (h * 3.0 / 8.0, h * 3.0 / 8.0),
                '\u{2584}' => (h * 4.0 / 8.0, h * 4.0 / 8.0),
                '\u{2585}' => (h * 5.0 / 8.0, h * 5.0 / 8.0),
                '\u{2586}' => (h * 6.0 / 8.0, h * 6.0 / 8.0),
                '\u{2587}' => (h * 7.0 / 8.0, h * 7.0 / 8.0),
                '\u{2594}' => (h * 1.0 / 8.0, h),
                '\u{1fb82}' => (h * 2.0 / 8.0, h),
                '\u{1fb83}' => (h * 3.0 / 8.0, h),
                '\u{1fb84}' => (h * 5.0 / 8.0, h),
                '\u{1fb85}' => (h * 6.0 / 8.0, h),
                '\u{1fb86}' => (h * 7.0 / 8.0, h),
                _ => (h, h),
            };
            let y = (h - y).round();
            rect_width = rect_width.round().max(1.0);
            rect_height = rect_height.round().max(1.0);
            let x = match c {
                '\u{2590}' => g.x_center(),
                '\u{2595}' | '\u{1fb87}'..='\u{1fb8b}' => w - rect_width,
                _ => 0.0,
            };
            g.rect(x, y, rect_width, rect_height);
        }
        // 整块与浓度块 '█','░','▒','▓'（alpha 与旧灰度 64/128/192 一致）。
        '\u{2588}' | '\u{2591}' | '\u{2592}' | '\u{2593}' => {
            let alpha = match c {
                '\u{2588}' => 1.0,
                '\u{2591}' => 64.0 / 255.0,
                '\u{2592}' => 128.0 / 255.0,
                '\u{2593}' => 192.0 / 255.0,
                _ => unreachable!(),
            };
            g.rect_alpha(0.0, 0.0, w, h, alpha);
        }
        // 象限块 '▖'..'▟'。
        '\u{2596}'..='\u{259f}' => {
            let x_center = g.x_center().round().max(1.0);
            let y_center = g.y_center().round().max(1.0);

            // 左上象限。
            let (w_second, h_second) = match c {
                '\u{2598}' | '\u{2599}' | '\u{259a}' | '\u{259b}' | '\u{259c}' => {
                    (x_center, y_center)
                }
                _ => (0.0, 0.0),
            };
            // 右上象限。
            let (w_first, h_first) = match c {
                '\u{259b}' | '\u{259c}' | '\u{259d}' | '\u{259e}' | '\u{259f}' => {
                    (x_center, y_center)
                }
                _ => (0.0, 0.0),
            };
            // 左下象限。
            let (w_third, h_third) = match c {
                '\u{2596}' | '\u{2599}' | '\u{259b}' | '\u{259e}' | '\u{259f}' => {
                    (x_center, y_center)
                }
                _ => (0.0, 0.0),
            };
            // 右下象限。
            let (w_fourth, h_fourth) = match c {
                '\u{2597}' | '\u{2599}' | '\u{259a}' | '\u{259c}' | '\u{259f}' => {
                    (x_center, y_center)
                }
                _ => (0.0, 0.0),
            };

            g.rect(0.0, 0.0, w_second, h_second);
            g.rect(x_center, 0.0, w_first, h_first);
            g.rect(0.0, y_center, w_third, h_third);
            g.rect(x_center, y_center, w_fourth, h_fourth);
        }
        // 六分块 '🬀'..'🬻'（2×3 网格，表与旧实现逐字一致）。
        '\u{1fb00}'..='\u{1fb3b}' => {
            let x_center = g.x_center().round().max(1.0);
            let y_third = (h / 3.0).round().max(1.0);
            let y_last_third = h - 2.0 * y_third;

            let (w_top_left, h_top_left) = match c {
                '\u{1fb00}' | '\u{1fb02}' | '\u{1fb04}' | '\u{1fb06}' | '\u{1fb08}'
                | '\u{1fb0a}' | '\u{1fb0c}' | '\u{1fb0e}' | '\u{1fb10}' | '\u{1fb12}'
                | '\u{1fb15}' | '\u{1fb17}' | '\u{1fb19}' | '\u{1fb1b}' | '\u{1fb1d}'
                | '\u{1fb1f}' | '\u{1fb21}' | '\u{1fb23}' | '\u{1fb25}' | '\u{1fb27}'
                | '\u{1fb28}' | '\u{1fb2a}' | '\u{1fb2c}' | '\u{1fb2e}' | '\u{1fb30}'
                | '\u{1fb32}' | '\u{1fb34}' | '\u{1fb36}' | '\u{1fb38}' | '\u{1fb3a}' => {
                    (x_center, y_third)
                }
                _ => (0.0, 0.0),
            };
            let (w_top_right, h_top_right) = match c {
                '\u{1fb01}' | '\u{1fb02}' | '\u{1fb05}' | '\u{1fb06}' | '\u{1fb09}'
                | '\u{1fb0a}' | '\u{1fb0d}' | '\u{1fb0e}' | '\u{1fb11}' | '\u{1fb12}'
                | '\u{1fb14}' | '\u{1fb15}' | '\u{1fb18}' | '\u{1fb19}' | '\u{1fb1c}'
                | '\u{1fb1d}' | '\u{1fb20}' | '\u{1fb21}' | '\u{1fb24}' | '\u{1fb25}'
                | '\u{1fb28}' | '\u{1fb2b}' | '\u{1fb2c}' | '\u{1fb2f}' | '\u{1fb30}'
                | '\u{1fb33}' | '\u{1fb34}' | '\u{1fb37}' | '\u{1fb38}' | '\u{1fb3b}' => {
                    (x_center, y_third)
                }
                _ => (0.0, 0.0),
            };
            let (w_mid_left, h_mid_left) = match c {
                '\u{1fb03}' | '\u{1fb04}' | '\u{1fb05}' | '\u{1fb06}' | '\u{1fb0b}'
                | '\u{1fb0c}' | '\u{1fb0d}' | '\u{1fb0e}' | '\u{1fb13}' | '\u{1fb14}'
                | '\u{1fb15}' | '\u{1fb1a}' | '\u{1fb1b}' | '\u{1fb1c}' | '\u{1fb1d}'
                | '\u{1fb22}' | '\u{1fb23}' | '\u{1fb24}' | '\u{1fb25}' | '\u{1fb29}'
                | '\u{1fb2a}' | '\u{1fb2b}' | '\u{1fb2c}' | '\u{1fb31}' | '\u{1fb32}'
                | '\u{1fb33}' | '\u{1fb34}' | '\u{1fb39}' | '\u{1fb3a}' | '\u{1fb3b}' => {
                    (x_center, y_third)
                }
                _ => (0.0, 0.0),
            };
            let (w_mid_right, h_mid_right) = match c {
                '\u{1fb07}' | '\u{1fb08}' | '\u{1fb09}' | '\u{1fb0a}' | '\u{1fb0b}'
                | '\u{1fb0c}' | '\u{1fb0d}' | '\u{1fb0e}' | '\u{1fb16}' | '\u{1fb17}'
                | '\u{1fb18}' | '\u{1fb19}' | '\u{1fb1a}' | '\u{1fb1b}' | '\u{1fb1c}'
                | '\u{1fb1d}' | '\u{1fb26}' | '\u{1fb27}' | '\u{1fb28}' | '\u{1fb29}'
                | '\u{1fb2a}' | '\u{1fb2b}' | '\u{1fb2c}' | '\u{1fb35}' | '\u{1fb36}'
                | '\u{1fb37}' | '\u{1fb38}' | '\u{1fb39}' | '\u{1fb3a}' | '\u{1fb3b}' => {
                    (x_center, y_third)
                }
                _ => (0.0, 0.0),
            };
            let (w_bottom_left, h_bottom_left) = match c {
                '\u{1fb0f}' | '\u{1fb10}' | '\u{1fb11}' | '\u{1fb12}' | '\u{1fb13}'
                | '\u{1fb14}' | '\u{1fb15}' | '\u{1fb16}' | '\u{1fb17}' | '\u{1fb18}'
                | '\u{1fb19}' | '\u{1fb1a}' | '\u{1fb1b}' | '\u{1fb1c}' | '\u{1fb1d}'
                | '\u{1fb2d}' | '\u{1fb2e}' | '\u{1fb2f}' | '\u{1fb30}' | '\u{1fb31}'
                | '\u{1fb32}' | '\u{1fb33}' | '\u{1fb34}' | '\u{1fb35}' | '\u{1fb36}'
                | '\u{1fb37}' | '\u{1fb38}' | '\u{1fb39}' | '\u{1fb3a}' | '\u{1fb3b}' => {
                    (x_center, y_last_third)
                }
                _ => (0.0, 0.0),
            };
            let (w_bottom_right, h_bottom_right) = match c {
                '\u{1fb1e}' | '\u{1fb1f}' | '\u{1fb20}' | '\u{1fb21}' | '\u{1fb22}'
                | '\u{1fb23}' | '\u{1fb24}' | '\u{1fb25}' | '\u{1fb26}' | '\u{1fb27}'
                | '\u{1fb28}' | '\u{1fb29}' | '\u{1fb2a}' | '\u{1fb2b}' | '\u{1fb2c}'
                | '\u{1fb2d}' | '\u{1fb2e}' | '\u{1fb2f}' | '\u{1fb30}' | '\u{1fb31}'
                | '\u{1fb32}' | '\u{1fb33}' | '\u{1fb34}' | '\u{1fb35}' | '\u{1fb36}'
                | '\u{1fb37}' | '\u{1fb38}' | '\u{1fb39}' | '\u{1fb3a}' | '\u{1fb3b}' => {
                    (x_center, y_last_third)
                }
                _ => (0.0, 0.0),
            };

            g.rect(0.0, 0.0, w_top_left, h_top_left);
            g.rect(x_center, 0.0, w_top_right, h_top_right);
            g.rect(0.0, y_third, w_mid_left, h_mid_left);
            g.rect(x_center, y_third, w_mid_right, h_mid_right);
            g.rect(0.0, y_third * 2.0, w_bottom_left, h_bottom_left);
            g.rect(x_center, y_third * 2.0, w_bottom_right, h_bottom_right);
        }
        // Powerline 实心三角 '',''：上下各留 1 设备像素（同旧实现），
        // 斜率 1；尖端放不下时在边缘截平成梯形。
        '\u{e0b0}' | '\u{e0b2}' => {
            let top = 1.0;
            let bottom = h - 1.0;
            let apex_x = (h - 2.0) / 2.0;
            let mut points = if apex_x <= w {
                vec![[0.0, top], [apex_x, h / 2.0], [0.0, bottom]]
            } else {
                vec![[0.0, top], [w, top + w], [w, bottom - w], [0.0, bottom]]
            };
            if c == '\u{e0b2}' {
                for p in &mut points {
                    p[0] = w - p[0];
                }
            }
            g.poly(points);
        }
        // Powerline 箭头 '',''：两道竖向厚度 = stroke 的斜笔画；
        // 尖端被截平时补一段竖桥封口（同旧实现的封口行为）。
        '\u{e0b1}' | '\u{e0b3}' => {
            let t = stroke;
            let top = 1.0;
            let bottom = h - 1.0;
            let apex_x = ((h - 2.0) / 2.0).min(w);
            let apex_top_y = top + apex_x;
            let apex_bot_y = bottom - apex_x;
            let mut polys = vec![
                vec![
                    [0.0, top],
                    [apex_x, apex_top_y],
                    [apex_x, apex_top_y + t],
                    [0.0, top + t],
                ],
                vec![
                    [0.0, bottom],
                    [apex_x, apex_bot_y],
                    [apex_x, apex_bot_y - t],
                    [0.0, bottom - t],
                ],
            ];
            if apex_x >= w && apex_top_y + t < apex_bot_y - t {
                polys.push(vec![
                    [apex_x - t, apex_top_y],
                    [apex_x, apex_top_y],
                    [apex_x, apex_bot_y],
                    [apex_x - t, apex_bot_y],
                ]);
            }
            for mut points in polys {
                if c == '\u{e0b3}' {
                    for p in &mut points {
                        p[0] = w - p[0];
                    }
                }
                g.poly(points);
            }
        }
        // Powerline 圆头 '',''：半椭圆（凸），折线近似。
        '\u{e0b4}' | '\u{e0b6}' => {
            let (flat_x, dir) = if c == '\u{e0b4}' {
                (0.0, 1.0)
            } else {
                (w, -1.0)
            };
            let (rx, ry) = (w, h / 2.0);
            const SEGMENTS: usize = 16;
            let mut points = Vec::with_capacity(SEGMENTS + 1);
            for i in 0..=SEGMENTS {
                let a = -FRAC_PI_2 + PI * i as f32 / SEGMENTS as f32;
                points.push([flat_x + dir * rx * a.cos(), h / 2.0 + ry * a.sin()]);
            }
            g.poly(points);
        }
        _ => unreachable!("is_builtin 与 draw 的覆盖必须一致: {c:?}"),
    }
}
