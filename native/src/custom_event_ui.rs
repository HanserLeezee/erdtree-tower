//! 问号房四大新增随机事件的 ImGui 交互面板与生存倒计时 HUD：
//! - 余烬熔炉（EmberFurnace）：免费重铸 / 灰烬拾遗
//! - 骨灰融合炉（SpiritFurnace）：吞噬强化 / 双灰融魂 / 唤灵拾遗
//! - 癫火祭坛（FrenzyAltar）：燃尽圣杯 / 献血成狂 / 癫火烙印 / 拒绝离开
//! - 「坚持 30 秒」大混战（Survival）：开战确认卡片 + 顶部实时倒计时与击杀 HUD
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use hudhook::imgui::{Condition, ImColor32, Key, Ui, WindowFlags};
use erdtree_tower::{
    Run,
    embers::Ember,
    new_events::{
        self, EMBER_FIND_PRICE, FRENZY_BLOOD_GOLD, FRENZY_BRAND_GOLD,
        FrenzyPact, SPIRIT_BOOST_PRICE, SPIRIT_FIND_PRICE, SPIRIT_FUSE_PRICE, SURVIVAL_BASE_GOLD,
        SURVIVAL_COUNT, SURVIVAL_KILL_GOLD, SURVIVAL_SECONDS,
    },
};

pub type EventKey = (u64, u8, usize);

#[derive(Clone)]
pub struct EmberFurnaceState {
    pub key: EventKey,
    pub runes: u32,
    pub owned: Vec<Ember>,
    pub spirits: bool,
    pub casting: bool,
    pub run_snapshot: Run,
}

#[derive(Clone)]
pub struct SpiritEntry {
    pub id: u32,
    pub base: u32,
    pub level: u8,
    pub name: String,
}

#[derive(Clone)]
pub struct SpiritFurnaceState {
    pub key: EventKey,
    pub runes: u32,
    pub spirits: Vec<SpiritEntry>,
    pub run_snapshot: Run,
}

#[derive(Clone)]
pub struct FrenzyAltarState {
    pub key: EventKey,
    pub hp: i32,
    pub max_hp: i32,
    pub flask_limit: u8,
    pub can_burn_flask: bool,
    pub blood_embers: Vec<Ember>,
}

#[derive(Clone)]
pub struct SurvivalOfferState {
    pub key: EventKey,
    pub party: bool,
    pub host: bool,
}

#[derive(Clone, Debug)]
pub enum Request {
    EmberFurnace {
        key: EventKey,
        burned: Option<Ember>,
        picked: Ember,
        price: u32,
    },
    SpiritDevour {
        key: EventKey,
        main_id: u32,
        sacrifice_id: u32,
        target_id: u32,
    },
    SpiritFuse {
        key: EventKey,
        sacrifice_a: u32,
        sacrifice_b: u32,
        target_id: u32,
        price: u32,
    },
    SpiritBoost {
        key: EventKey,
        main_id: u32,
        target_id: u32,
        price: u32,
    },
    SpiritFind {
        key: EventKey,
        target_id: u32,
        price: u32,
    },
    FrenzyAltar {
        key: EventKey,
        pact: FrenzyPact,
        ember_pick: Option<Ember>,
    },
    SurvivalStart {
        key: EventKey,
    },
    SurvivalSkip {
        key: EventKey,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct SurvivalHud {
    pub remaining_secs: f32,
    pub kills: u8,
    pub total: u8,
    pub preparing_secs: Option<u64>,
}

enum Panel {
    Ember {
        data: EmberFurnaceState,
        /// 0 = 免费重铸, 1 = 灰烬拾遗
        mode: usize,
        burn_idx: usize,
        step_pick: bool,
    },
    Spirit {
        data: SpiritFurnaceState,
        /// 0 = 吞噬强化, 1 = 双灰融魂, 2 = 唤灵拾遗 / 单灰强化
        mode: usize,
        primary: usize,
        secondary: usize,
    },
    Frenzy {
        data: FrenzyAltarState,
        picking_blood_ember: bool,
    },
    Survival(SurvivalOfferState),
}

static ACTIVE: AtomicBool = AtomicBool::new(false);
static PANEL: Mutex<Option<Panel>> = Mutex::new(None);
static PENDING: Mutex<Option<Request>> = Mutex::new(None);
static SURVIVAL_HUD: Mutex<Option<SurvivalHud>> = Mutex::new(None);

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

pub fn close() {
    if let Ok(mut slot) = PANEL.lock() {
        *slot = None;
    }
    ACTIVE.store(false, Ordering::Release);
}

pub fn take_request() -> Option<Request> {
    PENDING.lock().ok()?.take()
}

pub fn set_survival_hud(hud: Option<SurvivalHud>) {
    if let Ok(mut slot) = SURVIVAL_HUD.lock() {
        *slot = hud;
    }
}

pub fn open_ember_furnace(data: EmberFurnaceState) {
    let mode = if data.owned.is_empty() { 1 } else { 0 };
    if let Ok(mut slot) = PANEL.lock() {
        *slot = Some(Panel::Ember {
            data,
            mode,
            burn_idx: 0,
            step_pick: false,
        });
    }
    ACTIVE.store(true, Ordering::Release);
}

pub fn open_spirit_furnace(data: SpiritFurnaceState) {
    let mode = if data.spirits.len() >= 2 { 0 } else { 2 };
    let secondary = usize::from(data.spirits.len() >= 2);
    if let Ok(mut slot) = PANEL.lock() {
        *slot = Some(Panel::Spirit {
            data,
            mode,
            primary: 0,
            secondary,
        });
    }
    ACTIVE.store(true, Ordering::Release);
}

pub fn open_frenzy_altar(data: FrenzyAltarState) {
    if let Ok(mut slot) = PANEL.lock() {
        *slot = Some(Panel::Frenzy {
            data,
            picking_blood_ember: false,
        });
    }
    ACTIVE.store(true, Ordering::Release);
}

pub fn open_survival_offer(data: SurvivalOfferState) {
    if let Ok(mut slot) = PANEL.lock() {
        *slot = Some(Panel::Survival(data));
    }
    ACTIVE.store(true, Ordering::Release);
}

fn submit(req: Request) {
    if let Ok(mut slot) = PENDING.lock() {
        *slot = Some(req);
    }
    close();
}

const GOLD: [f32; 4] = [0.91, 0.72, 0.30, 1.0];
const WHITE: [f32; 4] = [0.94, 0.92, 0.88, 1.0];
const MUTED: [f32; 4] = [0.65, 0.62, 0.56, 1.0];
const RED: [f32; 4] = [0.92, 0.35, 0.28, 1.0];
const GREEN: [f32; 4] = [0.55, 0.86, 0.55, 1.0];
const PURPLE: [f32; 4] = [0.78, 0.56, 0.96, 1.0];

fn col(c: [f32; 4], a: f32) -> ImColor32 {
    ImColor32::from_rgba(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        (c[3] * a.clamp(0.0, 1.0) * 255.0) as u8,
    )
}

fn tab_button(ui: &Ui, label: &str, selected: bool, enabled: bool, w: f32, h: f32) -> bool {
    let pos = ui.cursor_screen_pos();
    let clicked = ui.invisible_button(label, [w, h]) && enabled;
    let hovered = enabled && ui.is_item_hovered();
    let draw = ui.get_window_draw_list();
    let bg = if !enabled {
        col([0.12, 0.11, 0.10, 1.0], 0.65)
    } else if selected {
        col(GOLD, 0.24)
    } else if hovered {
        col([0.24, 0.21, 0.17, 1.0], 0.85)
    } else {
        col([0.14, 0.13, 0.12, 1.0], 0.85)
    };
    let border = if !enabled {
        col(MUTED, 0.25)
    } else if selected {
        col(GOLD, 0.95)
    } else if hovered {
        col(GOLD, 0.55)
    } else {
        col(WHITE, 0.18)
    };
    draw.add_rect(pos, [pos[0] + w, pos[1] + h], bg).filled(true).rounding(5.0).build();
    draw.add_rect(pos, [pos[0] + w, pos[1] + h], border).rounding(5.0).thickness(if selected { 2.0 } else { 1.0 }).build();
    let display = label.split("##").next().unwrap_or(label);
    let ts = ui.calc_text_size(display);
    ui.set_cursor_screen_pos([pos[0] + (w - ts[0]) * 0.5, pos[1] + (h - ts[1]) * 0.5]);
    ui.text_colored(if !enabled { MUTED } else if selected { GOLD } else { WHITE }, display);
    ui.set_cursor_screen_pos([pos[0], pos[1] + h]);
    clicked
}

fn action_button(ui: &Ui, label: &str, accent: [f32; 4], enabled: bool, w: f32, h: f32) -> bool {
    let pos = ui.cursor_screen_pos();
    let clicked = ui.invisible_button(label, [w, h]) && enabled;
    let hovered = enabled && ui.is_item_hovered();
    let draw = ui.get_window_draw_list();
    let bg = if !enabled {
        col([0.15, 0.14, 0.13, 1.0], 0.7)
    } else if hovered {
        col(accent, 0.95)
    } else {
        col(accent, 0.78)
    };
    draw.add_rect(pos, [pos[0] + w, pos[1] + h], bg).filled(true).rounding(5.0).build();
    draw.add_rect(pos, [pos[0] + w, pos[1] + h], col(accent, if enabled { 1.0 } else { 0.3 })).rounding(5.0).thickness(1.5).build();
    let display = label.split("##").next().unwrap_or(label);
    let ts = ui.calc_text_size(display);
    ui.set_cursor_screen_pos([pos[0] + (w - ts[0]) * 0.5, pos[1] + (h - ts[1]) * 0.5]);
    ui.text_colored(if enabled { [0.07, 0.07, 0.06, 1.0] } else { MUTED }, display);
    ui.set_cursor_screen_pos([pos[0], pos[1] + h]);
    clicked
}

pub fn draw_hud(ui: &Ui, top_y: f32) -> f32 {
    let Some(hud) = SURVIVAL_HUD.lock().ok().and_then(|s| *s) else {
        return top_y;
    };
    let size = ui.io().display_size;
    let s = (size[1] / 1080.0).clamp(0.65, 1.8);
    let text = if let Some(prep) = hud.preparing_secs {
        format!("「坚持 30 秒」大混战 · 敌人将在 {prep} 秒后涌入")
    } else {
        format!(
            "「坚持 30 秒」大混战 · 剩余 {:.1} 秒 · 已击杀 {}/{}",
            hud.remaining_secs.max(0.0),
            hud.kills,
            hud.total
        )
    };
    let color = if hud.preparing_secs.is_some() {
        [232, 196, 120]
    } else if hud.remaining_secs <= 5.0 {
        [140, 232, 140]
    } else {
        [244, 152, 112]
    };
    let px = 24.0 * s;
    let w = crate::fancy_text::measure(&text, px) + 44.0 * s;
    let h = 44.0 * s;
    let x = (size[0] - w) * 0.5;
    let y = top_y;
    let draw = ui.get_foreground_draw_list();
    draw.add_rect([x, y], [x + w, y + h], ImColor32::from_rgba(12, 10, 9, 210))
        .filled(true)
        .rounding(6.0 * s)
        .build();
    draw.add_rect(
        [x, y],
        [x + w, y + h],
        ImColor32::from_rgba(color[0], color[1], color[2], 190),
    )
    .rounding(6.0 * s)
    .thickness(1.5 * s)
    .build();
    drop(draw);
    crate::fancy_text::draw(
        crate::fancy_text::Layer::Foreground,
        [x + 22.0 * s, y + 9.0 * s],
        px,
        &text,
        &crate::fancy_text::Style {
            color,
            accent: color,
            effect: crate::fancy_text::Effect::Plain,
            glow: false,
        },
        0.0,
        1.0,
    );
    top_y + h + 8.0 * s
}

/// 绘制全屏事件交互面板；返回 true 表示当前占着屏幕并接管输入。
pub fn draw(ui: &Ui) -> bool {
    if !active() {
        return false;
    }
    let cancel = ui.is_key_pressed_no_repeat(Key::Escape) || ui.is_key_pressed_no_repeat(Key::GamepadFaceRight);
    let mut should_close = false;
    let mut outgoing: Option<Request> = None;

    let size = ui.io().display_size;
    let s = (size[0] / 1600.0).min(size[1] / 900.0).clamp(0.65, 1.6);
    let bg = ui.get_background_draw_list();
    bg.add_rect([0.0, 0.0], size, ImColor32::from_rgba(10, 9, 8, 205))
        .filled(true)
        .build();
    drop(bg);

    let card_w = (1020.0 * s).min(size[0] * 0.92);
    let card_h = (680.0 * s).min(size[1] * 0.88);
    let left = (size[0] - card_w) * 0.5;
    let top = (size[1] - card_h) * 0.5;

    let Ok(mut guard) = PANEL.lock() else {
        ACTIVE.store(false, Ordering::Release);
        return false;
    };
    let Some(panel) = guard.as_mut() else {
        ACTIVE.store(false, Ordering::Release);
        return false;
    };

    ui.window("##tower-custom-event")
        .position([left, top], Condition::Always)
        .size([card_w, card_h], Condition::Always)
        .flags(
            WindowFlags::NO_DECORATION
                | WindowFlags::NO_BACKGROUND
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_NAV,
        )
        .build(|| {
            let draw = ui.get_window_draw_list();
            crate::law_ui::panel(&draw, [left, top], [left + card_w, top + card_h], s);
            drop(draw);

            let pad = 34.0 * s;
            let inner_w = card_w - pad * 2.0;
            ui.set_window_font_scale(s);

            match panel {
                Panel::Ember {
                    data,
                    mode,
                    burn_idx,
                    step_pick,
                } => {
                    ui.set_cursor_screen_pos([left + pad, top + pad]);
                    ui.text_colored(GOLD, "余烬熔炉");
                    ui.same_line();
                    ui.set_cursor_screen_pos([left + card_w - pad - 220.0 * s, top + pad]);
                    ui.text_colored(MUTED, format!("持有卢恩：{}", data.runes));

                    ui.set_cursor_screen_pos([left + pad, top + pad + 32.0 * s]);
                    ui.text_colored(MUTED, "将不再需要的余烬投入炉火重铸，或直接花卢恩从灰烬中拾取新余烬（每处限用一次）。");

                    let tab_w = (inner_w - 12.0 * s) / 2.0;
                    let tab_y = top + pad + 66.0 * s;
                    let has_owned = !data.owned.is_empty();
                    let tabs = [
                        ("余烬重铸（免费）##ef0", has_owned),
                        ("灰烬拾遗（2000 卢恩）##ef1", data.runes >= EMBER_FIND_PRICE),
                    ];
                    for (i, (label, enabled)) in tabs.iter().enumerate() {
                        ui.set_cursor_screen_pos([left + pad + i as f32 * (tab_w + 12.0 * s), tab_y]);
                        if tab_button(ui, label, *mode == i, *enabled, tab_w, 42.0 * s) {
                            *mode = i;
                            *step_pick = false;
                        }
                    }

                    let body_y = tab_y + 58.0 * s;
                    if !*step_pick {
                        let mut y = body_y;
                        if *mode == 0 {
                            ui.set_cursor_screen_pos([left + pad, y]);
                            ui.text_colored(WHITE, "选择要投入熔炉销毁的已有余烬：");
                            y += 28.0 * s;
                            if *burn_idx >= data.owned.len() {
                                *burn_idx = 0;
                            }
                            let cols = 3usize;
                            let bw = (inner_w - 20.0 * s) / cols as f32;
                            for (i, &ember) in data.owned.iter().enumerate().take(9) {
                                let cx = left + pad + (i % cols) as f32 * (bw + 10.0 * s);
                                let cy = y + (i / cols) as f32 * 44.0 * s;
                                ui.set_cursor_screen_pos([cx, cy]);
                                let lbl = format!("{}##burn{i}", ember.name());
                                if tab_button(ui, &lbl, *burn_idx == i, true, bw, 36.0 * s) {
                                    *burn_idx = i;
                                }
                            }
                            let rows = data.owned.len().min(9).div_ceil(cols).max(1);
                            y += rows as f32 * 44.0 * s + 10.0 * s;
                            if let Some(&e) = data.owned.get(*burn_idx) {
                                ui.set_cursor_screen_pos([left + pad, y]);
                                ui.text_colored(MUTED, format!("已选销毁：{} — {}", e.name(), e.effect().replace('\n', " / ")));
                            }
                        } else {
                            ui.set_cursor_screen_pos([left + pad, y]);
                            ui.text_colored(WHITE, "无需销毁已有余烬：花费 2000 卢恩直接开启随机余烬三选一。");
                        }

                        let price = if *mode == 0 { 0 } else { EMBER_FIND_PRICE };
                        let can_roll = data.runes >= price && (*mode == 1 || !data.owned.is_empty());
                        let btn_y = top + card_h - pad - 52.0 * s;
                        ui.set_cursor_screen_pos([left + pad, btn_y]);
                        if action_button(ui, "投入熔炉，抽取 3 选 1##roll_ember", GOLD, can_roll, 280.0 * s, 46.0 * s) {
                            *step_pick = true;
                        }
                        ui.set_cursor_screen_pos([left + card_w - pad - 180.0 * s, btn_y]);
                        if tab_button(ui, "暂不操作 [Esc]##close_ef", false, true, 180.0 * s, 46.0 * s) {
                            should_close = true;
                        }
                    } else {
                        let burned = (*mode == 0).then(|| data.owned.get(*burn_idx).copied()).flatten();
                        let price = if *mode == 0 { 0 } else { EMBER_FIND_PRICE };
                        let offers = new_events::ember_furnace_offers(
                            &data.run_snapshot,
                            data.key.2,
                            burned,
                            data.spirits,
                            data.casting,
                        );
                        ui.set_cursor_screen_pos([left + pad, body_y]);
                        ui.text_colored(GOLD, "请选择一枚新余烬（确认后立即扣除费用并替换）：");
                        let cw = (inner_w - 28.0 * s) / 3.0;
                        let ch = 310.0 * s;
                        let cy = body_y + 38.0 * s;
                        for (i, &ember) in offers.iter().enumerate() {
                            let cx = left + pad + i as f32 * (cw + 14.0 * s);
                            let q_col = if ember.quality() >= 3 { GOLD } else { PURPLE };
                            let draw = ui.get_window_draw_list();
                            draw.add_rect([cx, cy], [cx + cw, cy + ch], col([0.11, 0.10, 0.09, 1.0], 0.94))
                                .filled(true)
                                .rounding(6.0)
                                .build();
                            draw.add_rect([cx, cy], [cx + cw, cy + ch], col(q_col, 0.75))
                                .rounding(6.0)
                                .thickness(1.5)
                                .build();
                            drop(draw);
                            ui.set_cursor_screen_pos([cx + 16.0 * s, cy + 16.0 * s]);
                            ui.text_colored(q_col, format!("[{}] {}", if ember.quality() >= 3 { "传说" } else { "史诗" }, ember.name()));
                            ui.set_cursor_screen_pos([cx + 16.0 * s, cy + 54.0 * s]);
                            let _wrap = ui.push_text_wrap_pos_with_pos(cx + cw - 16.0 * s);
                            ui.text_colored(WHITE, ember.effect());
                            drop(_wrap);
                            ui.set_cursor_screen_pos([cx + 16.0 * s, cy + ch - 54.0 * s]);
                            let lbl = format!("选择此余烬##pick_e{i}");
                            if action_button(ui, &lbl, q_col, true, cw - 32.0 * s, 40.0 * s) {
                                outgoing = Some(Request::EmberFurnace {
                                    key: data.key,
                                    burned,
                                    picked: ember,
                                    price,
                                });
                            }
                        }
                        if offers.is_empty() {
                            ui.set_cursor_screen_pos([left + pad, cy + 20.0 * s]);
                            ui.text_colored(RED, "当前构筑已集齐全部可用余烬，无法继续抽取。");
                        }
                        let btn_y = top + card_h - pad - 52.0 * s;
                        ui.set_cursor_screen_pos([left + pad, btn_y]);
                        if tab_button(ui, "返回上一步##back_ef", false, true, 180.0 * s, 46.0 * s) {
                            *step_pick = false;
                        }
                        ui.set_cursor_screen_pos([left + card_w - pad - 180.0 * s, btn_y]);
                        if tab_button(ui, "取消离开 [Esc]##cancel_ef", false, true, 180.0 * s, 46.0 * s) {
                            should_close = true;
                        }
                    }
                }

                Panel::Spirit {
                    data,
                    mode,
                    primary,
                    secondary,
                } => {
                    ui.set_cursor_screen_pos([left + pad, top + pad]);
                    ui.text_colored(GOLD, "骨灰融合炉");
                    ui.same_line();
                    ui.set_cursor_screen_pos([left + card_w - pad - 220.0 * s, top + pad]);
                    ui.text_colored(MUTED, format!("持有卢恩：{}", data.runes));

                    ui.set_cursor_screen_pos([left + pad, top + pad + 32.0 * s]);
                    ui.text_colored(MUTED, "吞噬闲置骨灰直接大幅强化，或双灰融魂锻造高阶定制骨灰（每处限用一次）。");

                    let tab_w = (inner_w - 24.0 * s) / 3.0;
                    let tab_y = top + pad + 66.0 * s;
                    let has_two = data.spirits.len() >= 2;
                    let tabs = [
                        ("吞噬强化（免费，+3）##sf0", has_two),
                        ("双灰融魂（1000 卢恩）##sf1", has_two && data.runes >= SPIRIT_FUSE_PRICE),
                        ("唤灵拾遗 / 单灰强化##sf2", true),
                    ];
                    for (i, (label, enabled)) in tabs.iter().enumerate() {
                        ui.set_cursor_screen_pos([left + pad + i as f32 * (tab_w + 12.0 * s), tab_y]);
                        if tab_button(ui, label, *mode == i, *enabled, tab_w, 42.0 * s) {
                            *mode = i;
                        }
                    }

                    let mut y = tab_y + 58.0 * s;
                    if *mode == 0 || *mode == 1 {
                        if *primary >= data.spirits.len() {
                            *primary = 0;
                        }
                        if *secondary >= data.spirits.len() || *secondary == *primary {
                            *secondary = (0..data.spirits.len()).find(|&i| i != *primary).unwrap_or(0);
                        }
                        let col_w = (inner_w - 20.0 * s) * 0.5;
                        ui.set_cursor_screen_pos([left + pad, y]);
                        ui.text_colored(GOLD, if *mode == 0 { "选择主骨灰（保留并 +3）" } else { "选择祭品骨灰 A（销毁）" });
                        ui.set_cursor_screen_pos([left + pad + col_w + 20.0 * s, y]);
                        ui.text_colored(RED, if *mode == 0 { "选择祭品骨灰（销毁）" } else { "选择祭品骨灰 B（销毁）" });
                        y += 28.0 * s;
                        for (i, sp) in data.spirits.iter().enumerate().take(8) {
                            let row_y = y + i as f32 * 40.0 * s;
                            let lbl_a = format!("{} +{}##spa{i}", sp.name, sp.level);
                            ui.set_cursor_screen_pos([left + pad, row_y]);
                            if tab_button(ui, &lbl_a, *primary == i, true, col_w, 34.0 * s) {
                                *primary = i;
                                if *secondary == i {
                                    *secondary = (0..data.spirits.len()).find(|&k| k != i).unwrap_or(0);
                                }
                            }
                            let lbl_b = format!("{} +{}##spb{i}", sp.name, sp.level);
                            ui.set_cursor_screen_pos([left + pad + col_w + 20.0 * s, row_y]);
                            if tab_button(ui, &lbl_b, *secondary == i, i != *primary, col_w, 34.0 * s) {
                                *secondary = i;
                            }
                        }

                        let btn_y = top + card_h - pad - 52.0 * s;
                        let owned_bases: Vec<u32> = data.spirits.iter().map(|s| s.base).collect();
                        if *mode == 0 {
                            let target = data.spirits.get(*primary).and_then(|s| new_events::spirit_devour_target(s.id));
                            let can = target.is_some() && *primary != *secondary && data.spirits.len() >= 2;
                            ui.set_cursor_screen_pos([left + pad, btn_y - 30.0 * s]);
                            if let (Some(main), Some(sac), Some(t)) = (data.spirits.get(*primary), data.spirits.get(*secondary), target) {
                                let nl = erdtree_tower::rewards::spirit_level(t).unwrap_or(main.level);
                                ui.text_colored(GREEN, format!("结果：{} +{} → +{}（消耗 {} +{}）", main.name, main.level, nl, sac.name, sac.level));
                            } else {
                                ui.text_colored(RED, "主骨灰已达最高等级，无法继续吞噬强化。");
                            }
                            ui.set_cursor_screen_pos([left + pad, btn_y]);
                            if action_button(ui, "确认吞噬强化（免费）##do_devour", GOLD, can, 280.0 * s, 46.0 * s)
                                && let Some(t) = target
                            {
                                outgoing = Some(Request::SpiritDevour {
                                    key: data.key,
                                    main_id: data.spirits[*primary].id,
                                    sacrifice_id: data.spirits[*secondary].id,
                                    target_id: t,
                                });
                            }
                        } else {
                            let target = data.spirits.get(*primary).zip(data.spirits.get(*secondary)).and_then(|(a, b)| {
                                new_events::spirit_fusion_result(&data.run_snapshot, data.key.2, a.id, b.id, &owned_bases)
                            });
                            let can = target.is_some() && *primary != *secondary && data.runes >= SPIRIT_FUSE_PRICE;
                            ui.set_cursor_screen_pos([left + pad, btn_y - 30.0 * s]);
                            if let (Some(a), Some(b), Some(t)) = (data.spirits.get(*primary), data.spirits.get(*secondary), target) {
                                let nl = erdtree_tower::rewards::spirit_level(t).unwrap_or(0);
                                ui.text_colored(PURPLE, format!("结果：销毁 {} 与 {} → 融合出 1 只全新高阶/定制骨灰（+{}）", a.name, b.name, nl));
                            }
                            ui.set_cursor_screen_pos([left + pad, btn_y]);
                            if action_button(ui, "确认双灰融魂（1000 卢恩）##do_fuse", PURPLE, can, 300.0 * s, 46.0 * s)
                                && let Some(t) = target
                            {
                                outgoing = Some(Request::SpiritFuse {
                                    key: data.key,
                                    sacrifice_a: data.spirits[*primary].id,
                                    sacrifice_b: data.spirits[*secondary].id,
                                    target_id: t,
                                    price: SPIRIT_FUSE_PRICE,
                                });
                            }
                        }
                        ui.set_cursor_screen_pos([left + card_w - pad - 180.0 * s, btn_y]);
                        if tab_button(ui, "暂不操作 [Esc]##close_sf", false, true, 180.0 * s, 46.0 * s) {
                            should_close = true;
                        }
                    } else {
                        let owned_bases: Vec<u32> = data.spirits.iter().map(|s| s.base).collect();
                        let find_target = new_events::spirit_find_result(&data.run_snapshot, data.key.2, &owned_bases);
                        let boost_target = data.spirits.first().and_then(|s| new_events::spirit_boost_target(s.id));

                        ui.set_cursor_screen_pos([left + pad, y]);
                        ui.text_colored(WHITE, "即使背包骨灰不足 2 只，也可在此补充骨灰或直接强化：");
                        y += 40.0 * s;

                        ui.set_cursor_screen_pos([left + pad, y]);
                        if action_button(
                            ui,
                            "唤灵拾遗：随机获得 1 只当前等级骨灰（1000 卢恩）##do_find_sp",
                            GOLD,
                            find_target.is_some() && data.runes >= SPIRIT_FIND_PRICE,
                            520.0 * s,
                            52.0 * s,
                        ) && let Some(t) = find_target
                        {
                            outgoing = Some(Request::SpiritFind {
                                key: data.key,
                                target_id: t,
                                price: SPIRIT_FIND_PRICE,
                            });
                        }
                        y += 72.0 * s;
                        if let Some(first) = data.spirits.first() {
                            let lbl = format!("灵灰淬炼：将 {} +{} 直接 +2（1200 卢恩）##do_boost_sp", first.name, first.level);
                            ui.set_cursor_screen_pos([left + pad, y]);
                            if action_button(
                                ui,
                                &lbl,
                                GREEN,
                                boost_target.is_some() && data.runes >= SPIRIT_BOOST_PRICE,
                                520.0 * s,
                                52.0 * s,
                            ) && let Some(t) = boost_target
                            {
                                outgoing = Some(Request::SpiritBoost {
                                    key: data.key,
                                    main_id: first.id,
                                    target_id: t,
                                    price: SPIRIT_BOOST_PRICE,
                                });
                            }
                        }

                        let btn_y = top + card_h - pad - 52.0 * s;
                        ui.set_cursor_screen_pos([left + card_w - pad - 180.0 * s, btn_y]);
                        if tab_button(ui, "暂不操作 [Esc]##close_sf2", false, true, 180.0 * s, 46.0 * s) {
                            should_close = true;
                        }
                    }
                }

                Panel::Frenzy {
                    data,
                    picking_blood_ember,
                } => {
                    ui.set_cursor_screen_pos([left + pad, top + pad]);
                    ui.text_colored(RED, "癫火祭坛");
                    ui.same_line();
                    ui.set_cursor_screen_pos([left + card_w - pad - 340.0 * s, top + pad]);
                    ui.text_colored(MUTED, format!("当前生命 {}/{} · 圣杯瓶上限 {}", data.hp, data.max_hp, data.flask_limit));

                    ui.set_cursor_screen_pos([left + pad, top + pad + 32.0 * s]);
                    ui.text_colored(MUTED, "癫火从不索要卢恩，它只收割你的容器、鲜血或理智。可签下一项契约，也可转身离开。");

                    if !*picking_blood_ember {
                        let cw = (inner_w - 28.0 * s) / 3.0;
                        let ch = 360.0 * s;
                        let cy = top + pad + 82.0 * s;
                        let cards = [
                            (
                                "燃尽圣杯",
                                "代价：本局永久 -1 红圣杯瓶上限\n（最低保底 2 瓶）",
                                "收益：\n• 1 件传说词条武器\n• 1 个高阶 / 被动词条护符",
                                data.can_burn_flask,
                                GOLD,
                            ),
                            (
                                "献血成狂",
                                "代价：立刻失去当前 80% 生命\n（至少保留 1 点生命）",
                                "收益：\n• 立刻开启随机余烬三选一\n• 2000 卢恩",
                                true,
                                RED,
                            ),
                            (
                                "癫火烙印",
                                "代价：下一场战斗受到伤害 +35%\n（赢下该场战斗后自动解除）",
                                "收益：\n• 3500 卢恩\n• 1 件传说词条武器",
                                true,
                                PURPLE,
                            ),
                        ];
                        for (i, (title, cost, gain, enabled, accent)) in cards.iter().enumerate() {
                            let cx = left + pad + i as f32 * (cw + 14.0 * s);
                            let draw = ui.get_window_draw_list();
                            draw.add_rect([cx, cy], [cx + cw, cy + ch], col([0.11, 0.09, 0.09, 1.0], 0.94))
                                .filled(true)
                                .rounding(6.0)
                                .build();
                            draw.add_rect([cx, cy], [cx + cw, cy + ch], col(*accent, if *enabled { 0.8 } else { 0.25 }))
                                .rounding(6.0)
                                .thickness(1.5)
                                .build();
                            drop(draw);
                            ui.set_cursor_screen_pos([cx + 18.0 * s, cy + 18.0 * s]);
                            ui.text_colored(*accent, *title);
                            let _w = ui.push_text_wrap_pos_with_pos(cx + cw - 18.0 * s);
                            ui.set_cursor_screen_pos([cx + 18.0 * s, cy + 56.0 * s]);
                            ui.text_colored(RED, *cost);
                            ui.set_cursor_screen_pos([cx + 18.0 * s, cy + 150.0 * s]);
                            ui.text_colored(WHITE, *gain);
                            drop(_w);
                            ui.set_cursor_screen_pos([cx + 18.0 * s, cy + ch - 56.0 * s]);
                            let lbl = format!("签下契约##frenzy{i}");
                            if action_button(ui, &lbl, *accent, *enabled, cw - 36.0 * s, 42.0 * s) {
                                match i {
                                    0 => {
                                        outgoing = Some(Request::FrenzyAltar {
                                            key: data.key,
                                            pact: FrenzyPact::BurnFlask,
                                            ember_pick: None,
                                        });
                                    }
                                    1 => {
                                        if data.blood_embers.is_empty() {
                                            outgoing = Some(Request::FrenzyAltar {
                                                key: data.key,
                                                pact: FrenzyPact::BloodMadness,
                                                ember_pick: None,
                                            });
                                        } else {
                                            *picking_blood_ember = true;
                                        }
                                    }
                                    _ => {
                                        outgoing = Some(Request::FrenzyAltar {
                                            key: data.key,
                                            pact: FrenzyPact::FrenzyBrand,
                                            ember_pick: None,
                                        });
                                    }
                                }
                            }
                        }
                        let btn_y = top + card_h - pad - 52.0 * s;
                        ui.set_cursor_screen_pos([left + card_w - pad - 200.0 * s, btn_y]);
                        if tab_button(ui, "拒绝离开 [Esc]##close_fr", false, true, 200.0 * s, 46.0 * s) {
                            should_close = true;
                        }
                    } else {
                        ui.set_cursor_screen_pos([left + pad, top + pad + 78.0 * s]);
                        ui.text_colored(RED, format!("献血成狂：选择一枚余烬（同时扣除当前 80% 生命并获得 {} 卢恩）", FRENZY_BLOOD_GOLD));
                        let cw = (inner_w - 28.0 * s) / 3.0;
                        let ch = 310.0 * s;
                        let cy = top + pad + 118.0 * s;
                        for (i, &ember) in data.blood_embers.iter().enumerate() {
                            let cx = left + pad + i as f32 * (cw + 14.0 * s);
                            let q_col = if ember.quality() >= 3 { GOLD } else { PURPLE };
                            let draw = ui.get_window_draw_list();
                            draw.add_rect([cx, cy], [cx + cw, cy + ch], col([0.11, 0.10, 0.09, 1.0], 0.94))
                                .filled(true)
                                .rounding(6.0)
                                .build();
                            draw.add_rect([cx, cy], [cx + cw, cy + ch], col(q_col, 0.8))
                                .rounding(6.0)
                                .thickness(1.5)
                                .build();
                            drop(draw);
                            ui.set_cursor_screen_pos([cx + 16.0 * s, cy + 16.0 * s]);
                            ui.text_colored(q_col, ember.name());
                            ui.set_cursor_screen_pos([cx + 16.0 * s, cy + 54.0 * s]);
                            let _w = ui.push_text_wrap_pos_with_pos(cx + cw - 16.0 * s);
                            ui.text_colored(WHITE, ember.effect());
                            drop(_w);
                            ui.set_cursor_screen_pos([cx + 16.0 * s, cy + ch - 54.0 * s]);
                            let lbl = format!("献血领取##pick_fe{i}");
                            if action_button(ui, &lbl, RED, true, cw - 32.0 * s, 40.0 * s) {
                                outgoing = Some(Request::FrenzyAltar {
                                    key: data.key,
                                    pact: FrenzyPact::BloodMadness,
                                    ember_pick: Some(ember),
                                });
                            }
                        }
                        let btn_y = top + card_h - pad - 52.0 * s;
                        ui.set_cursor_screen_pos([left + pad, btn_y]);
                        if tab_button(ui, "返回契约列表##back_fr", false, true, 180.0 * s, 46.0 * s) {
                            *picking_blood_ember = false;
                        }
                        let _ = FRENZY_BRAND_GOLD;
                    }
                }

                Panel::Survival(data) => {
                    ui.set_cursor_screen_pos([left + pad, top + pad]);
                    ui.text_colored(GOLD, format!("「坚持 {} 秒」大混战", SURVIVAL_SECONDS));
                    let _w = ui.push_text_wrap_pos_with_pos(left + card_w - pad);
                    ui.set_cursor_screen_pos([left + pad, top + pad + 48.0 * s]);
                    ui.text_colored(
                        WHITE,
                        format!(
                            "场中将同时涌入 {} 名敌人（2 名精英 + 6 名杂兵）。\n\
                            • 只要存活满 {} 秒（联机时全队至少 1 人存活），残留敌人立即自动清场并结算奖励！\n\
                            • 保底奖励：{} 卢恩 + 1 件史诗/传说装备\n\
                            • 击杀加码：每多击杀 1 名敌人额外 +{} 卢恩\n\
                            • 30 秒内全歼 {}/{}：升级为必出传说词条武器 + 1 个高阶/被动护符！",
                            SURVIVAL_COUNT,
                            SURVIVAL_SECONDS,
                            SURVIVAL_BASE_GOLD,
                            SURVIVAL_KILL_GOLD,
                            SURVIVAL_COUNT,
                            SURVIVAL_COUNT
                        ),
                    );
                    if data.party && !data.host {
                        ui.set_cursor_screen_pos([left + pad, top + pad + 260.0 * s]);
                        ui.text_colored(GOLD, "联机提示：任意队员点击接受挑战，全队将同步开战。");
                    }
                    drop(_w);

                    let btn_y = top + card_h - pad - 56.0 * s;
                    ui.set_cursor_screen_pos([left + pad, btn_y]);
                    if action_button(ui, "接受 30 秒大混战挑战##start_surv", RED, true, 320.0 * s, 48.0 * s) {
                        outgoing = Some(Request::SurvivalStart { key: data.key });
                    }
                    ui.set_cursor_screen_pos([left + pad + 340.0 * s, btn_y]);
                    if tab_button(ui, "放弃挑战并离开（无奖励）##skip_surv", false, true, 260.0 * s, 48.0 * s) {
                        outgoing = Some(Request::SurvivalSkip { key: data.key });
                    }
                    ui.set_cursor_screen_pos([left + card_w - pad - 160.0 * s, btn_y]);
                    if tab_button(ui, "先再想想 [Esc]##close_surv", false, true, 160.0 * s, 48.0 * s) {
                        should_close = true;
                    }
                }
            }
            ui.set_window_font_scale(1.0);
        });

    drop(guard);
    if let Some(req) = outgoing {
        submit(req);
    } else if should_close || cancel {
        close();
    }
    true
}
