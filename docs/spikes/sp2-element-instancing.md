# SP-2 GPUI 커스텀 Element 인스턴싱

- 수행일: 2026-10-11
- 코드: `spikes/sp2-element-instancing/` (독립 cargo 프로젝트, 빈 `[workspace]`)
- GPUI: crates.io `gpui-kit = "=0.7.1"`(`default-features = false`). gpui-kit이 re-export하는 `gpui-pre =0.3.8`만 쓴다. path 의존은 쓰지 않았다.

## 질문

자체 `Element`로 사각형 10,000개(노트)를 60 fps로 그릴 수 있는가? `paint`에서 쓸 수 있는 프리미티브는 무엇이고, Metal에서 배치·인스턴싱되는가?

## 방법

### 1. 소스 분석 (`~/.cargo/registry/src/index.crates.io-*/`)

- `paint`에서 쓰는 `Window` 프리미티브(`gpui-pre-0.3.8/src/window.rs`): `paint_quad`(4550), `paint_path`(4621), `paint_underline`/`paint_strikethrough`(4638/4652), `paint_glyph`/`paint_emoji`(4687/4792), `paint_svg`(4849), `paint_image`(4919), `paint_surface`(5021, CVPixelBuffer), `paint_drop_shadows`/`paint_inset_shadows`(4424/4460), `paint_layer`(4399), `with_content_mask`(4029). quad 생성 도우미 `fill`/`quad`/`outline`(7675-7716). 모서리 반경·테두리·테두리 스타일은 quad 하나에 들어간다.
- 장면 구성(`scene.rs`): `insert_primitive`(87)는 레이어 밖이면 primitive마다 `BoundsTree::insert`(`bounds_tree.rs:120`, 겹치는 기존 bounds의 최대 order + 1)로 그리기 순서를 정한다. `paint_layer` 안이면 레이어의 order 하나를 그대로 써서 BoundsTree 삽입을 건너뛴다(`push_layer` 75-80). `finish`(151)에서 종류별로 order 정렬.
- 배치(`scene.rs:291-370` `BatchIterator`): 종류가 같은 primitive는 다른 종류가 order 사이에 끼기 전까지 한 배치(`PrimitiveBatch::Quads(range)`)로 묶인다.
- Metal(`gpui-pre-apple-0.3.8/src/metal_renderer.rs`): `draw_quads`(898)는 배치 하나를 `draw_primitives_instanced_base_instance(Triangle, 0, 6, quads.len(), quads.start)`(931) 호출 한 번으로 그린다. 즉 quad 배치는 인스턴스 버퍼(초기 2 MB, 최대 256 MB, `metal_renderer.rs:39,62,1523`)에 올린 인스턴싱 draw call 한 번이다. 셰이더는 `shaders.metal`.

### 2. 실험 (`spikes/sp2-element-instancing`)

- `Element` trait을 직접 구현한 `NotesElement`. 노트는 틱 순 정렬 `Vec`이고 `paint`에서 `partition_point`로 화면 왼쪽 끝을 찾은 뒤 오른쪽 끝에서 멈춘다(S3-T1 리뷰 초점의 정렬 캐시 전제). 노트 하나 = `paint_quad(quad(bounds, 2px 반경, 벨로시티 명도 배경, 1px 테두리))`.
- 노트를 32마디 안에 고르게 뿌리고, 줌을 「전체가 들어가는 배율」의 1.0~1.3배로, 가로·세로 스크롤과 행 높이를 사인파로 계속 바꾼다. 매 프레임 `request_animation_frame`. 노트 대부분이 항상 화면 안에 있는 최악에 가까운 조건이다(프레임당 실제 그린 노트 수를 함께 기록).
- 모드 두 가지: `layer`(노트 전체를 `paint_layer` 하나로 감쌈), `nolayer`(`paint_quad`만).
- 측정(프로그램 내, Instruments 미사용): 프레임 간격(`render` 호출 간격), `paint` 소요, `render` 시작~`paint` 끝(레이아웃·prepaint 포함 CPU). 첫 1초는 워밍업으로 버리고 8초 측정 후 스스로 종료. 드롭 프레임 = 간격 > 25 ms(60 Hz 1.5프레임).
- 창 1600x1000 pt(배율 2x, 3200x2000 px).

실행:

~~~
cd spikes/sp2-element-instancing
cargo build --release
./target/release/sp2-element-instancing layer 8 10000      # [layer|nolayer] [초] [노트 수] [마디 수=32]
./target/release/sp2-element-instancing nolayer 8 10000
~~~

측정 환경: MacBook Pro(Mac13,1, Apple M1 Max 24코어 GPU, 32 GB), macOS 27.0.1(26A434), 외장 LG 40" 5K2K(2560x1080 @ 60 Hz, 배율 2x), rustc 1.95.0, release 프로필(`debug = 1`).

## 결과

| 노트 수 | 모드 | 프레임당 그린 노트(평균) | 평균 fps | 프레임 간격 p50 / p99 / max (ms) | paint p50 / p95 / max (ms) | render~paint 끝 p95 (ms) | 드롭 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 10,000 | layer | 8,851 | 60.0 | 16.66 / 18.00 / 23.29 | 0.97 / 1.93 / 2.51 | 2.01 | 0/367 |
| 10,000 | nolayer | 9,034 | 60.0 | 16.67 / 17.19 / 17.59 | 6.08 / 7.51 / 8.20 | 7.53 | 0/499 |
| 10,000 (320마디) | layer | 8,847 | 60.0 | 16.67 / 17.90 / 19.09 | 0.98 / 1.93 / 2.98 | 2.00 | 0/492 |
| 50,000 | layer | 45,304 | 60.0 | 16.67 / 17.10 / 17.61 | 2.32 / 3.16 / 3.73 | 3.17 | 0/487 |
| 50,000 | nolayer | 44,710 | 15.8 | 62.4 / 92.7 / 98.0 | 59.4 / 78.8 / 94.3 | 78.8 | 129/129 |
| 100,000 | layer | 90,382 | 60.0 | 16.67 / 17.42 / 17.64 | 3.64 / 3.85 / 3.96 | 3.87 | 0/489 |
| 100,000 | nolayer | 89,087 | 6.8 | 147 / 196 / 207 | 135 / 185 / 198 | 185 | 57/57 |

- 노트 10,000개는 두 모드 모두 60 fps, 드롭 0%(NF-8 충족). 다만 `nolayer`는 paint만으로 프레임 예산의 절반(6~8 ms)을 쓰고, 노트 수에 대해 초선형으로 늘어 5만 개에서 무너진다. 원인은 primitive마다 하는 `BoundsTree::insert`다(GPU 쪽은 두 모드 모두 quad 배치 하나).
- `paint_layer`로 감싸면 10만 개도 paint 4 ms 미만, 60 fps를 유지한다. 피아노롤 노트는 서로 겹쳐 그리는 순서가 중요하지 않으므로 레이어 하나에 넣어도 의미가 바뀌지 않는다. 레이어 안에서는 같은 order끼리 삽입 순서대로 그려진다(`finish`의 안정 정렬).
- GPU 시간은 따로 재지 않았다. 60 Hz 디스플레이에서 프레임 간격이 vsync에 붙어 있으므로 GPU가 병목이 아니라는 것까지만 말할 수 있다.

### 확인 못 함 / 주의

- 120 Hz(ProMotion) 화면에서는 측정하지 않았다. 10,000개 기준 CPU 2 ms 수준이라 8.3 ms 예산에도 들 것으로 보이나 미확인.
- 노트 위 글자(음 이름 등)는 `paint_glyph`(MonochromeSprite)라 quad와 다른 배치가 된다. 레이어 안에서 quad와 글리프가 같은 order면 `BatchIterator`가 (order, 종류)로 정렬해 quad 배치 하나 + 스프라이트 배치 하나로 묶일 것으로 보이나, 글자를 그린 측정은 하지 않았다.
- `paint_path`(CC 곡선 등)는 배치마다 렌더 패스를 끊고 중간 텍스처를 쓴다(`metal_renderer.rs` `PrimitiveBatch::Paths` 처리). 컨트롤러 레인의 곡선 비용은 별도 확인이 필요하다.
- 프레임 간격은 `render` 호출 간격이라 실제 화면 표시 간격의 근사다.

## 계획에 미치는 영향

S3-T1 「결정 분기」 중 (A) GPUI 프리미티브를 택한다. 자체 Metal 패스(B)는 필요 없다.

- GPUI의 `paint_quad`는 이미 Metal 인스턴싱 draw call 한 번으로 배치되고, 노트 10,000개 화면에서 60 fps·드롭 0%를 확인했다.
- 조건: 노트 quad는 `paint_layer`로 감싼다. 레이어 없이 그리면 10,000개에서 paint 6~8 ms로 예산의 절반을 쓴다. S3-T1의 criterion 기준값과 리뷰 초점에 「노트 quad는 `paint_layer` 안에서 그린다」를 넣을 것을 권한다.
- S3-T1 테스트 게이트(10,000 노트 paint 16.7 ms 안)는 현재 수치(p95 약 2 ms)로 여유가 크다. 배치 뷰·컨트롤러 레인·Sequence 레인도 같은 방식(정렬 캐시 + 화면 범위 이진 탐색 + `paint_layer`)을 쓰면 된다.

사람이 확인할 항목: 120 Hz 내장 디스플레이에서 `layer 8 10000` 재실행(프레임 간격 p50 8.3 ms 유지, 드롭 0% 확인).
