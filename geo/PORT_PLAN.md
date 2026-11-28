# GeoPolit: перенос с WebView (JS) на Rust + Kotlin

Цель: вся игра без WebView. Источник логики — `geo/src/main/assets/index.html` (старая JS-версия, удалить в конце вместе с d3/topojson/earcut/raster.js/glworker.js).

## Архитектура
- `geo/native/` — Rust crate `geopolit` (cdylib, JNI). Всё ядро:
  - `data.rs` — загрузка TopoJSON (world_lo/mid/world, water, marine), nations, cities; триангуляция (earcutr), соседи регионов, растр REG 4096x2048.
  - `render.rs` — OpenGL ES 3 (glow): фон (bg.jpg из Kotlin как RGBA), заливка регионов (цвет владельца через текстуру), зоны оккупации (текстуры zid/field + шейдер со штриховкой), границы, вода, реки, фронты, дивизии, города, подписи (fontdue + атлас, шрифт из /system/fonts).
  - `game.rs` — состояние (serde), экономика dayTick, ИИ, дипломатия, сохранение.
  - `occ.rs` — оккупация клетками (CTL, zones), шлейф, сглаживание, карманы.
  - `divs.rs` — дивизии, gridRoute A*, движение по часовым тикам.
  - `front.rs` — фронты, план (область, полосы), покрытие/бреши, прорыв.
  - `peace.rs` — мирная конференция.
  - `lib.rs` — JNI: init(assets), surface, frame, touch, tap -> JSON, ui-запросы (JSON), действия.
- Kotlin (`geo/src/main/java/com/geopolit/game/`): Activity, GLSurfaceView (рендер в Rust), жесты, весь UI на Android Views (HUD, панели, меню, выбор страны, конференция, тулбар фронта), сохранение в файлы.
- CI: rustup target aarch64-linux-android (+armv7, x86_64), cargo-ndk -> `geo/src/main/jniLibs`.

## Статус
См. git log. Тесты ядра: `cargo test` в geo/native; рендер на десктопе через ANGLE/SwiftShader из Chromium (tools/shot).
