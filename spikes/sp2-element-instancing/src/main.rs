//! SP-2: 자체 `Element`로 노트 사각형 10,000개를 스크롤·줌하며 그릴 때의 프레임 시간 측정.
//!
//! 실행: `cargo run --release -- [layer|nolayer] [초] [노트 수] [마디 수]`
//! - `layer`: 노트 전체를 `paint_layer` 하나로 감싼다(모든 quad가 같은 order → BoundsTree 삽입 생략).
//! - `nolayer`: `paint_quad`만 호출(quad마다 BoundsTree에 넣어 order를 계산).
//! 측정이 끝나면 스스로 종료하고 결과를 stdout에 한 번 찍는다.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::*;

const PPQ: u32 = 960;
const PITCHES: u32 = 128;
const WARMUP: Duration = Duration::from_secs(1);

#[derive(Clone, Copy)]
struct Note {
    start: u32,
    len: u32,
    pitch: u8,
    velocity: u8,
}

/// 결정적인 의사 난수(재현 가능한 노트 배치). 외부 crate를 피하려고 xorshift를 쓴다.
fn make_notes(count: usize, bars: u32) -> Vec<Note> {
    let span = bars * PPQ * 4;
    let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    let mut notes: Vec<Note> = (0..count)
        .map(|i| {
            // `bars`마디(4/4) 안에 고르게 흩뿌리고 16분음표 격자에 맞춘다.
            let step = ((i as u64 * u64::from(span)) / count as u64) as u32 / (PPQ / 4) * (PPQ / 4);
            Note {
                start: step,
                len: PPQ / 4 * (1 + (next() % 4) as u32),
                pitch: (24 + next() % 72) as u8,
                velocity: (30 + next() % 97) as u8,
            }
        })
        .collect();
    // 피아노롤은 틱 순 정렬 캐시를 쓴다는 계획(S3-T1 리뷰 초점)을 따른다.
    notes.sort_by_key(|n| n.start);
    notes
}

#[derive(Default)]
struct Stats {
    paint_ms: Vec<f64>,
    frame_cpu_ms: Vec<f64>,
    interval_ms: Vec<f64>,
    painted_per_frame: Vec<usize>,
}

fn pct(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((s.len() - 1) as f64 * p).round() as usize;
    s[idx]
}

struct Shared {
    stats: Stats,
    started: Instant,
    last_render: Option<Instant>,
    render_started: Instant,
}

struct NotesElement {
    notes: Rc<Vec<Note>>,
    shared: Rc<RefCell<Shared>>,
    use_layer: bool,
    /// 가로 스크롤(틱)과 줌(px/틱), 세로 스크롤(px).
    scroll_tick: f32,
    px_per_tick: f32,
    scroll_y: f32,
    row_h: f32,
    measuring: bool,
}

impl IntoElement for NotesElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for NotesElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        let t0 = Instant::now();
        let mut painted = 0usize;
        let notes = self.notes.clone();
        let origin = bounds.origin;
        let w = f32::from(bounds.size.width);
        let h = f32::from(bounds.size.height);
        let first_tick = self.scroll_tick;
        let last_tick = first_tick + w / self.px_per_tick;

        let mut draw = |window: &mut Window| {
            window.paint_quad(fill(bounds, rgb(0x1e1f24)));
            // 화면 밖 노트는 이진 탐색으로 건너뛴다(정렬 캐시 전제). 길이 상한은 4분음표 4개.
            let max_len = (PPQ * 4) as f32;
            let lo = notes.partition_point(|n| (n.start as f32) + max_len < first_tick);
            for n in &notes[lo..] {
                let s = n.start as f32;
                if s > last_tick {
                    break;
                }
                let x = (s - first_tick) * self.px_per_tick;
                let y = (PITCHES - 1 - u32::from(n.pitch)) as f32 * self.row_h - self.scroll_y;
                if y + self.row_h < 0. || y > h {
                    continue;
                }
                let nw = (n.len as f32 * self.px_per_tick).max(1.);
                let b = Bounds::new(
                    point(origin.x + px(x), origin.y + px(y)),
                    size(px(nw), px(self.row_h - 1.)),
                );
                let l = 0.35 + f32::from(n.velocity) / 127. * 0.3;
                window.paint_quad(quad(
                    b,
                    px(2.),
                    hsla(0.58, 0.6, l, 1.),
                    px(1.),
                    hsla(0.58, 0.8, 0.85, 1.),
                    BorderStyle::Solid,
                ));
                painted += 1;
            }
        };

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if self.use_layer {
                window.paint_layer(bounds, |window| draw(window));
            } else {
                draw(window);
            }
        });

        let now = Instant::now();
        if self.measuring {
            let mut sh = self.shared.borrow_mut();
            let cpu = (now - sh.render_started).as_secs_f64() * 1e3;
            sh.stats.paint_ms.push((now - t0).as_secs_f64() * 1e3);
            sh.stats.frame_cpu_ms.push(cpu);
            sh.stats.painted_per_frame.push(painted);
        }
        // 다음 프레임을 계속 요청해 디스플레이 속도로 애니메이션한다.
        window.request_animation_frame();
    }
}

struct PianoRoll {
    notes: Rc<Vec<Note>>,
    shared: Rc<RefCell<Shared>>,
    use_layer: bool,
}

impl Render for PianoRoll {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let measuring;
        let t;
        {
            let mut sh = self.shared.borrow_mut();
            t = (now - sh.started).as_secs_f32();
            measuring = now - sh.started >= WARMUP;
            if let Some(prev) = sh.last_render
                && measuring
            {
                sh.stats.interval_ms.push((now - prev).as_secs_f64() * 1e3);
            }
            sh.last_render = Some(now);
            sh.render_started = now;
        }
        // 자동 스크롤·줌: 줌은 「전체가 들어가는 배율」의 1~1.3배를 오가고, 가로 스크롤은
        // 남는 폭 안에서 왕복한다. 노트 대부분이 항상 화면 안에 있도록 해 최악에 가깝게 잰다.
        let width = 1600.;
        let total_ticks = self.notes.last().map_or(1, |n| n.start + n.len) as f32;
        let fit = width / total_ticks;
        let zoom = 1. + 0.15 * (1. + (t * 0.7).sin());
        let px_per_tick = fit * zoom;
        let hidden_ticks = total_ticks - width / px_per_tick;
        let scroll_tick = hidden_ticks * 0.5 * (1. + (t * 0.9).sin());
        let row_h = 8. + 2. * (t * 0.5).cos();
        // 음역 24..96은 행 31..103. 위쪽 여백을 조금 두고 위아래로 흔든다.
        let scroll_y = 31. * row_h - 40. + 20. * (t * 1.3).sin();

        div().size_full().child(NotesElement {
            notes: self.notes.clone(),
            shared: self.shared.clone(),
            use_layer: self.use_layer,
            scroll_tick,
            px_per_tick,
            scroll_y,
            row_h,
            measuring,
        })
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let use_layer = args.get(1).map_or(true, |a| a != "nolayer");
    let secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(6);
    let count: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10_000);
    let bars: u32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(32);

    let notes = Rc::new(make_notes(count, bars));
    let shared = Rc::new(RefCell::new(Shared {
        stats: Stats::default(),
        started: Instant::now(),
        last_render: None,
        render_started: Instant::now(),
    }));

    gpui_kit::application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1600.), px(1000.)), cx);
        let shared_for_view = shared.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| PianoRoll {
                    notes: notes.clone(),
                    shared: shared_for_view,
                    use_layer,
                })
            },
        )
        .expect("open window");
        cx.activate(true);
        shared.borrow_mut().started = Instant::now();

        let shared_for_quit = shared.clone();
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_secs(secs) + WARMUP)
                .await;
            report(&shared_for_quit.borrow().stats, use_layer, count);
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}

fn report(s: &Stats, use_layer: bool, count: usize) {
    let frames = s.interval_ms.len();
    // 60 Hz 기준 1.5프레임(25 ms)을 넘는 간격을 드롭 프레임으로 센다. 16.7 ms 경계는
    // vsync 지터(±1 ms) 때문에 정상 프레임도 넘나든다.
    let dropped = s.interval_ms.iter().filter(|&&v| v > 25.0).count();
    let avg_interval = s.interval_ms.iter().sum::<f64>() / frames.max(1) as f64;
    let painted_avg =
        s.painted_per_frame.iter().sum::<usize>() as f64 / s.painted_per_frame.len().max(1) as f64;
    let painted_max = s.painted_per_frame.iter().copied().max().unwrap_or(0);
    let painted_min = s.painted_per_frame.iter().copied().min().unwrap_or(0);
    println!(
        "mode={} notes={} frames={} avg_fps={:.1}",
        if use_layer { "layer" } else { "nolayer" },
        count,
        frames,
        1000. / avg_interval
    );
    println!(
        "painted_per_frame avg={painted_avg:.0} min={painted_min} max={painted_max}"
    );
    for (name, v) in [
        ("frame_interval_ms", &s.interval_ms),
        ("paint_ms", &s.paint_ms),
        ("render_to_paint_end_ms", &s.frame_cpu_ms),
    ] {
        println!(
            "{name}: p50={:.3} p95={:.3} p99={:.3} max={:.3}",
            pct(v, 0.5),
            pct(v, 0.95),
            pct(v, 0.99),
            pct(v, 1.0)
        );
    }
    println!(
        "dropped(interval>25ms): {dropped}/{frames} ({:.2}%)",
        100. * dropped as f64 / frames.max(1) as f64
    );
}
