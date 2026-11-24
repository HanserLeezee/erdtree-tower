//! Native event dialogue choices and survival timer.
use erdtree_tower::{
    Run,
    embers::Ember,
    new_events::{
        self, EMBER_FIND_PRICE, FrenzyPact, SPIRIT_BOOST_PRICE, SPIRIT_FIND_PRICE,
        SPIRIT_FUSE_PRICE,
    },
};
use hudhook::imgui::{ImColor32, Ui};
use std::sync::Mutex;

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
    pub can_burn_flask: bool,
    pub blood_embers: Vec<Ember>,
}

#[derive(Clone)]
pub struct SurvivalOfferState {
    pub key: EventKey,
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

enum Event {
    Ember(EmberFurnaceState),
    Spirit(SpiritFurnaceState),
    Frenzy(FrenzyAltarState),
    Survival(SurvivalOfferState),
}

#[derive(Clone, Copy)]
enum Method {
    Devour,
    Fuse,
    Boost,
}

#[derive(Clone, Copy)]
enum Page {
    Root,
    Burn,
    Offers(Option<Ember>),
    Primary(Method),
    Secondary(Method, u32),
    Blood,
}

#[derive(Clone)]
enum Action {
    Go(Page),
    Confirm(i32, Request),
    Previous,
    Next,
    Back,
    Stay,
}
struct Session {
    event: Event,
    history: Vec<(Page, usize)>,
    choices: Vec<Action>,
    pending: Option<Request>,
}
const LABEL: i32 = 30_000_000;
const EMBER_NAME: i32 = 30_000_100;
const EMBER_INFO: i32 = 30_000_200;
pub(crate) fn ash_name(id: u32) -> i32 {
    100_000_000 + (id & 0x0fffffff) as i32
}
fn ember_name(e: Ember) -> i32 {
    EMBER_NAME + e.index() as i32
}
fn ember_info(e: Ember) -> i32 {
    EMBER_INFO + e.index() as i32
}

static MENU: Mutex<Option<Session>> = Mutex::new(None);
static PENDING: Mutex<Option<Request>> = Mutex::new(None);
static SURVIVAL_HUD: Mutex<Option<SurvivalHud>> = Mutex::new(None);
pub fn active() -> bool {
    MENU.lock().unwrap().is_some()
}
pub fn close() {
    *MENU.lock().unwrap() = None;
    *PENDING.lock().unwrap() = None;
}
pub fn take_request() -> Option<Request> {
    PENDING.lock().unwrap().take()
}
pub fn set_survival_hud(hud: Option<SurvivalHud>) {
    *SURVIVAL_HUD.lock().unwrap() = hud;
}
fn open(event: Event) {
    *MENU.lock().unwrap() = Some(Session {
        event,
        history: vec![(Page::Root, 0)],
        choices: vec![],
        pending: None,
    });
}
pub fn open_ember_furnace(data: EmberFurnaceState) {
    open(Event::Ember(data));
}
pub fn open_spirit_furnace(data: SpiritFurnaceState) {
    open(Event::Spirit(data));
}
pub fn open_frenzy_altar(data: FrenzyAltarState) {
    open(Event::Frenzy(data));
}
pub fn open_survival_offer(data: SurvivalOfferState) {
    open(Event::Survival(data));
}

impl Session {
    fn options(&self, page: Page) -> Vec<(i32, Action)> {
        use Action::{Confirm, Go, Stay};
        let mut rows = Vec::new();
        match (&self.event, page) {
            (Event::Ember(d), Page::Root) => {
                if !d.owned.is_empty() {
                    rows.push((LABEL, Go(Page::Burn)));
                }
                if d.runes >= EMBER_FIND_PRICE {
                    rows.push((LABEL + 1, Go(Page::Offers(None))));
                }
            }
            (Event::Ember(d), Page::Burn) => {
                for &e in &d.owned {
                    rows.push((ember_name(e), Go(Page::Offers(Some(e)))));
                }
            }
            (Event::Ember(d), Page::Offers(burned)) => {
                for picked in new_events::ember_furnace_offers(
                    &d.run_snapshot,
                    d.key.2,
                    burned,
                    d.spirits,
                    d.casting,
                ) {
                    rows.push((
                        ember_name(picked),
                        Confirm(
                            ember_info(picked) + if burned.is_some() { 0 } else { 100 },
                            Request::EmberFurnace {
                                key: d.key,
                                burned,
                                picked,
                                price: if burned.is_some() {
                                    0
                                } else {
                                    EMBER_FIND_PRICE
                                },
                            },
                        ),
                    ));
                }
            }
            (Event::Spirit(d), Page::Root) => {
                let bases = d.spirits.iter().map(|s| s.base).collect::<Vec<_>>();
                if d.spirits.len() >= 2 {
                    if d.spirits
                        .iter()
                        .any(|s| new_events::spirit_devour_target(s.id).is_some())
                    {
                        rows.push((LABEL + 2, Go(Page::Primary(Method::Devour))));
                    }
                    if d.runes >= SPIRIT_FUSE_PRICE {
                        rows.push((LABEL + 3, Go(Page::Primary(Method::Fuse))));
                    }
                }
                if d.runes >= SPIRIT_BOOST_PRICE
                    && d.spirits
                        .iter()
                        .any(|s| new_events::spirit_boost_target(s.id).is_some())
                {
                    rows.push((LABEL + 4, Go(Page::Primary(Method::Boost))));
                }
                if d.runes >= SPIRIT_FIND_PRICE
                    && let Some(target_id) =
                        new_events::spirit_find_result(&d.run_snapshot, d.key.2, &bases)
                {
                    rows.push((
                        LABEL + 5,
                        Confirm(
                            LABEL + 15,
                            Request::SpiritFind {
                                key: d.key,
                                target_id,
                                price: SPIRIT_FIND_PRICE,
                            },
                        ),
                    ));
                }
            }
            (Event::Spirit(d), Page::Primary(method)) => {
                for s in &d.spirits {
                    let action = match method {
                        Method::Devour if new_events::spirit_devour_target(s.id).is_none() => {
                            continue;
                        }
                        Method::Devour | Method::Fuse => Go(Page::Secondary(method, s.id)),
                        Method::Boost => {
                            let Some(target_id) = new_events::spirit_boost_target(s.id) else {
                                continue;
                            };
                            Confirm(
                                LABEL + 14,
                                Request::SpiritBoost {
                                    key: d.key,
                                    main_id: s.id,
                                    target_id,
                                    price: SPIRIT_BOOST_PRICE,
                                },
                            )
                        }
                    };
                    rows.push((ash_name(s.id), action));
                }
            }
            (Event::Spirit(d), Page::Secondary(method, main)) => {
                let bases = d.spirits.iter().map(|s| s.base).collect::<Vec<_>>();
                for s in d.spirits.iter().filter(|s| s.id != main) {
                    let action = match method {
                        Method::Devour => new_events::spirit_devour_target(main).map(|target_id| {
                            Confirm(
                                LABEL + 12,
                                Request::SpiritDevour {
                                    key: d.key,
                                    main_id: main,
                                    sacrifice_id: s.id,
                                    target_id,
                                },
                            )
                        }),
                        Method::Fuse => new_events::spirit_fusion_result(
                            &d.run_snapshot,
                            d.key.2,
                            main,
                            s.id,
                            &bases,
                        )
                        .map(|target_id| {
                            Confirm(
                                LABEL + 13,
                                Request::SpiritFuse {
                                    key: d.key,
                                    sacrifice_a: main,
                                    sacrifice_b: s.id,
                                    target_id,
                                    price: SPIRIT_FUSE_PRICE,
                                },
                            )
                        }),
                        Method::Boost => None,
                    };
                    if let Some(a) = action {
                        rows.push((ash_name(s.id), a));
                    }
                }
            }
            (Event::Frenzy(d), Page::Root) => {
                let pact = |pact| Request::FrenzyAltar {
                    key: d.key,
                    pact,
                    ember_pick: None,
                };
                if d.can_burn_flask {
                    rows.push((LABEL + 6, Confirm(LABEL + 16, pact(FrenzyPact::BurnFlask))));
                }
                rows.push((
                    LABEL + 7,
                    if d.blood_embers.is_empty() {
                        Confirm(LABEL + 17, pact(FrenzyPact::BloodMadness))
                    } else {
                        Go(Page::Blood)
                    },
                ));
                rows.push((
                    LABEL + 8,
                    Confirm(LABEL + 18, pact(FrenzyPact::FrenzyBrand)),
                ));
            }
            (Event::Frenzy(d), Page::Blood) => {
                for &e in &d.blood_embers {
                    rows.push((
                        ember_name(e),
                        Confirm(
                            ember_info(e) + 200,
                            Request::FrenzyAltar {
                                key: d.key,
                                pact: FrenzyPact::BloodMadness,
                                ember_pick: Some(e),
                            },
                        ),
                    ));
                }
            }
            (Event::Survival(d), Page::Root) => {
                rows.push((
                    LABEL + 9,
                    Confirm(LABEL + 19, Request::SurvivalStart { key: d.key }),
                ));
                rows.push((
                    LABEL + 10,
                    Confirm(LABEL + 20, Request::SurvivalSkip { key: d.key }),
                ));
            }
            _ => {}
        }
        if rows.is_empty() {
            rows.push((LABEL + 21, Stay));
        }
        rows
    }
    fn rows(&mut self) -> Vec<i32> {
        const PER_PAGE: usize = 6;
        let &(step, page) = self.history.last().unwrap();
        let options = self.options(step);
        let more = options.len() > (page + 1) * PER_PAGE;
        let mut visible: Vec<_> = options
            .into_iter()
            .skip(page * PER_PAGE)
            .take(PER_PAGE)
            .collect();
        if page > 0 {
            visible.push((LABEL + 22, Action::Previous));
        }
        if more {
            visible.push((LABEL + 23, Action::Next));
        }
        if self.history.len() > 1 {
            visible.push((LABEL + 24, Action::Back));
        }
        if matches!(step, Page::Secondary(..)) {
            visible.insert(0, (LABEL + 11, Action::Stay));
        }
        let (labels, actions) = visible.into_iter().unzip();
        self.choices = actions;
        labels
    }
}
pub fn rows() -> Option<Vec<i32>> {
    MENU.lock().unwrap().as_mut().map(Session::rows)
}
pub fn select(index: usize) -> Option<i32> {
    let mut slot = MENU.lock().unwrap();
    let menu = slot.as_mut()?;
    match menu.choices.get(index)?.clone() {
        Action::Go(page) => menu.history.push((page, 0)),
        Action::Back => {
            menu.history.pop();
        }
        Action::Next => menu.history.last_mut().unwrap().1 += 1,
        Action::Previous => menu.history.last_mut().unwrap().1 -= 1,
        Action::Stay => {}
        Action::Confirm(text, req) => {
            menu.pending = Some(req);
            return Some(text);
        }
    }
    None
}
pub fn confirm(yes: bool) {
    let mut slot = MENU.lock().unwrap();
    if let Some(request) = slot.as_mut().and_then(|s| s.pending.take())
        && yes
    {
        *PENDING.lock().unwrap() = Some(request);
        *slot = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decline_is_reversible_and_accept_submits_only_once() {
        close();
        let key = (7, 1, 3);
        open_survival_offer(SurvivalOfferState { key });
        assert_eq!(rows().unwrap(), [LABEL + 9, LABEL + 10]);
        assert_eq!(select(0), Some(LABEL + 19));
        confirm(false);
        assert!(take_request().is_none() && active());
        assert_eq!(select(1), Some(LABEL + 20));
        confirm(true);
        assert!(matches!(take_request(), Some(Request::SurvivalSkip { key: k }) if k == key));
        assert!(!active() && take_request().is_none());
    }

    #[test]
    fn inventory_pages_reach_every_ember() {
        let owned = Ember::ALL[..10].to_vec();
        let mut session = Session {
            event: Event::Ember(EmberFurnaceState {
                key: (1,1,1), runes: 0, owned: owned.clone(), spirits: false, casting: false,
                run_snapshot: Run::new(1, erdtree_tower::Class::Samurai),
            }),
            history: vec![(Page::Root,0),(Page::Burn,0)], choices: vec![], pending: None,
        };
        let first = session.rows();
        assert!(first.contains(&(LABEL + 23)));
        session.history.last_mut().unwrap().1 = 1;
        let second = session.rows();
        for e in owned { assert!(first.contains(&ember_name(e)) || second.contains(&ember_name(e))); }
        assert!(second.contains(&(LABEL + 22)) && second.contains(&(LABEL + 24)));
    }
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
