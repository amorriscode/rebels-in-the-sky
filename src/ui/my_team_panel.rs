use super::{
    button::Button,
    clickable_list::ClickableListState,
    clickable_table::{ClickableCell, ClickableRow, ClickableTable, ClickableTableState},
    constants::{PrintableKeyCode, UiKey, UiStyle},
    gif_map::GifMap,
    traits::{Screen, SplitPanel, StyledRating},
    ui_callback::{CallbackRegistry, UiCallbackPreset},
    utils::hover_text_target,
    widgets::{
        challenge_button, default_block, explore_button, go_to_team_current_planet_button,
        go_to_team_home_planet_button, render_player_description, render_spaceship_description,
        selectable_list, trade_button,
    },
};
use crate::{
    engine::game::Game,
    store::{load_from_json, PERSISTED_GAMES_PREFIX},
    types::{AppResult, GameId, PlayerId, SystemTimeTick, Tick},
    world::{
        constants::CURRENCY_SYMBOL,
        planet::PlanetType,
        position::{GamePosition, Position, MAX_POSITION},
        skill::Rated,
        types::TeamLocation,
        world::World,
    },
};
use crate::{
    types::{PlanetId, TeamId},
    world::{
        constants::{BASE_BONUS, BONUS_PER_SKILL},
        resources::Resource,
        role::CrewRole,
        skill::GameSkill,
    },
};
use core::fmt::Debug;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::{
    layout::Margin,
    prelude::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum MyTeamView {
    #[default]
    Info,
    Games,
    Market,
}

impl MyTeamView {
    fn next(&self) -> Self {
        match self {
            MyTeamView::Info => MyTeamView::Games,
            MyTeamView::Games => MyTeamView::Market,
            MyTeamView::Market => MyTeamView::Info,
        }
    }
}

#[derive(Debug, Default, PartialEq)]
enum PanelList {
    #[default]
    Top,
    Bottom,
}

#[derive(Debug, Default)]
pub struct MyTeamPanel {
    player_index: Option<usize>,
    game_index: Option<usize>,
    planet_index: Option<usize>,
    view: MyTeamView,
    active_list: PanelList,
    players: Vec<PlayerId>,
    recent_games: Vec<GameId>,
    loaded_games: HashMap<GameId, Game>,
    planet_markets: Vec<PlanetId>,
    challenge_teams: Vec<TeamId>,
    own_team_id: TeamId,
    current_planet_id: Option<PlanetId>,
    tick: usize,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
    gif_map: Arc<Mutex<GifMap>>,
}

impl MyTeamPanel {
    pub fn new(
        callback_registry: Arc<Mutex<CallbackRegistry>>,
        gif_map: Arc<Mutex<GifMap>>,
    ) -> Self {
        Self {
            callback_registry,
            gif_map,
            ..Default::default()
        }
    }

    fn render_view_buttons(&mut self, frame: &mut Frame, area: Rect) {
        let mut view_info_button = Button::new(
            "View: Info".into(),
            UiCallbackPreset::SetMyTeamPanelView {
                view: MyTeamView::Info,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW);

        let mut view_games_button = Button::new(
            "View: Games".into(),
            UiCallbackPreset::SetMyTeamPanelView {
                view: MyTeamView::Games,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW);

        let mut view_market_button = Button::new(
            "View: Market".into(),
            UiCallbackPreset::SetMyTeamPanelView {
                view: MyTeamView::Market,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW);

        match self.view {
            MyTeamView::Info => view_info_button.disable(None),
            MyTeamView::Games => view_games_button.disable(None),
            MyTeamView::Market => view_market_button.disable(None),
        }

        let split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

        frame.render_widget(view_info_button, split[0]);
        frame.render_widget(view_games_button, split[1]);
        frame.render_widget(view_market_button, split[2]);
    }

    fn render_market(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Min(20), Constraint::Length(54)]).split(area);
        self.render_market_buttons(frame, world, split[0])?;
        self.render_planet_markets(frame, world, split[1])?;

        Ok(())
    }

    fn render_planet_markets(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let team = world.get_own_team()?;
        frame.render_widget(default_block().title("Planet Markets"), area);
        let split = Layout::horizontal([Constraint::Length(20), Constraint::Length(30)]).split(
            area.inner(&Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        let mut options = vec![];
        for &id in self.planet_markets.iter() {
            let planet = world.get_planet_or_err(id)?;
            let text = planet.name.clone();
            let style = match team.current_location {
                TeamLocation::OnPlanet { planet_id } => {
                    if planet_id == planet.id {
                        UiStyle::OWN_TEAM
                    } else {
                        UiStyle::DEFAULT
                    }
                }
                _ => UiStyle::DEFAULT,
            };
            options.push((text, style));
        }

        let list = selectable_list(options, &self.callback_registry);

        frame.render_stateful_widget(
            list,
            split[0].inner(&Margin {
                horizontal: 1,
                vertical: 1,
            }),
            &mut ClickableListState::default().with_selected(self.planet_index),
        );

        let planet_id = self.planet_markets[self.planet_index.unwrap_or_default()];
        let planet = world.get_planet_or_err(planet_id)?;

        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("Resource: Buy {CURRENCY_SYMBOL}/Sell {CURRENCY_SYMBOL}"),
                    UiStyle::HEADER,
                )),
                Line::from(vec![
                    Span::styled("Fuel      ", UiStyle::STORAGE_FUEL),
                    Span::styled(
                        format!("{}", planet.resource_buy_price(Resource::FUEL)),
                        UiStyle::OK,
                    ),
                    Span::raw("/"),
                    Span::styled(
                        format!("{}", planet.resource_sell_price(Resource::FUEL)),
                        UiStyle::ERROR,
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Gold      ", UiStyle::STORAGE_GOLD),
                    Span::styled(
                        format!("{}", planet.resource_buy_price(Resource::GOLD)),
                        UiStyle::OK,
                    ),
                    Span::raw("/"),
                    Span::styled(
                        format!("{}", planet.resource_sell_price(Resource::GOLD)),
                        UiStyle::ERROR,
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Scraps    ", UiStyle::STORAGE_SCRAPS),
                    Span::styled(
                        format!("{}", planet.resource_buy_price(Resource::SCRAPS)),
                        UiStyle::OK,
                    ),
                    Span::raw("/"),
                    Span::styled(
                        format!("{}", planet.resource_sell_price(Resource::SCRAPS)),
                        UiStyle::ERROR,
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Rum       ", UiStyle::STORAGE_RUM),
                    Span::styled(
                        format!("{}", planet.resource_buy_price(Resource::RUM)),
                        UiStyle::OK,
                    ),
                    Span::raw("/"),
                    Span::styled(
                        format!("{}", planet.resource_sell_price(Resource::RUM)),
                        UiStyle::ERROR,
                    ),
                ]),
            ]),
            split[1],
        );

        Ok(())
    }

    fn render_market_buttons(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let team = world.get_own_team()?;
        frame.render_widget(default_block().title("Market"), area);

        let planet_id = match team.current_location {
            TeamLocation::OnPlanet { planet_id } => planet_id,
            TeamLocation::Travelling { .. } => {
                frame.render_widget(
                    Paragraph::new("There is no market available while travelling."),
                    area.inner(&Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );
                return Ok(());
            }
            TeamLocation::Exploring { .. } => {
                frame.render_widget(
                    Paragraph::new("There is no market available while exploring."),
                    area.inner(&Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );
                return Ok(());
            }
        };

        let planet = world.get_planet_or_err(planet_id)?;
        if planet.total_population() == 0 {
            frame.render_widget(
                Paragraph::new(
                    "There is no market available on this planet!\nTry another planet with more population.",
                ),
                area.inner(&Margin {
                    horizontal: 1,
                    vertical: 1,
                }),
            );
            return Ok(());
        }
        let hover_text_target = hover_text_target(frame);

        let button_split = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area.inner(&Margin {
            horizontal: 1,
            vertical: 1,
        }));

        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" Shortcuts                                       Buy {CURRENCY_SYMBOL}/Sell {CURRENCY_SYMBOL}"),
                UiStyle::HEADER,
            )),
            button_split[0],
        );

        let resource_styles = [
            UiStyle::STORAGE_FUEL,
            UiStyle::STORAGE_GOLD,
            UiStyle::STORAGE_SCRAPS,
            UiStyle::STORAGE_RUM,
        ];

        let buy_ui_keys = [
            UiKey::BUY_FUEL,
            UiKey::BUY_GOLD,
            UiKey::BUY_SCRAPS,
            UiKey::BUY_RUM,
        ];
        let sell_ui_keys = [
            UiKey::SELL_FUEL,
            UiKey::SELL_GOLD,
            UiKey::SELL_SCRAPS,
            UiKey::SELL_RUM,
        ];

        for (button_split_idx, resource) in [
            Resource::FUEL,
            Resource::GOLD,
            Resource::SCRAPS,
            Resource::RUM,
        ]
        .iter()
        .enumerate()
        {
            let resource_split = Layout::horizontal([
                Constraint::Length(12), // name
                Constraint::Max(6),     // buy 1
                Constraint::Max(6),     // buy 10
                Constraint::Max(6),     // buy 100
                Constraint::Max(6),     // sell 1
                Constraint::Max(6),     // sell 10
                Constraint::Max(6),     // sell 100
                Constraint::Min(0),     // price
            ])
            .split(button_split[button_split_idx + 1]);

            let buy_unit_cost = planet.resource_buy_price(*resource);
            let sell_unit_cost = planet.resource_sell_price(*resource);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        format!("{:<6} ", resource.to_string()),
                        resource_styles[button_split_idx],
                    ),
                    Span::styled(
                        format!("{}", buy_ui_keys[button_split_idx].to_string()),
                        UiStyle::OK,
                    ),
                    Span::raw(format!("/")),
                    Span::styled(
                        format!("{}", sell_ui_keys[button_split_idx].to_string()),
                        UiStyle::ERROR,
                    ),
                ])),
                resource_split[0].inner(&Margin {
                    horizontal: 1,
                    vertical: 1,
                }),
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!("{}", buy_unit_cost), UiStyle::OK),
                    Span::raw(format!("/")),
                    Span::styled(format!("{}", sell_unit_cost), UiStyle::ERROR),
                ])),
                resource_split[7].inner(&Margin {
                    horizontal: 1,
                    vertical: 1,
                }),
            );
            for (idx, amount) in [1, 10, 100].iter().enumerate() {
                if let Ok(btn) = trade_button(
                    &world,
                    resource.clone(),
                    amount.clone(),
                    buy_unit_cost,
                    &self.callback_registry,
                    hover_text_target,
                ) {
                    frame.render_widget(btn, resource_split[idx + 1]);
                }
                if let Ok(btn) = trade_button(
                    &world,
                    resource.clone(),
                    -amount.clone(),
                    sell_unit_cost,
                    &self.callback_registry,
                    hover_text_target,
                ) {
                    frame.render_widget(btn, resource_split[idx + 4]);
                }
            }
        }
        Ok(())
    }
    fn render_info(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let team = world.get_own_team()?;
        let hover_text_target = hover_text_target(&frame);

        let split = Layout::horizontal([Constraint::Max(48), Constraint::Min(32)]).split(area);
        let bars_length = 25;
        let mut gold_length = ((Resource::GOLD.to_storing_space()
            * team.resources.get(&Resource::GOLD).unwrap_or(&0).clone())
            as f32
            / team.spaceship.storage_capacity() as f32
            * bars_length as f32)
            .round() as usize;
        let mut scraps_length = ((Resource::SCRAPS.to_storing_space()
            * team.resources.get(&Resource::SCRAPS).unwrap_or(&0).clone())
            as f32
            / team.spaceship.storage_capacity() as f32
            * bars_length as f32)
            .round() as usize;
        let mut rum_length = ((Resource::RUM.to_storing_space()
            * team.resources.get(&Resource::RUM).unwrap_or(&0).clone())
            as f32
            / team.spaceship.storage_capacity() as f32
            * bars_length as f32)
            .round() as usize;

        let mut free_bars = bars_length - gold_length - scraps_length - rum_length;
        let free_space = team.spaceship.storage_capacity() - team.used_storage_capacity();
        // Try to round up to eliminate free bars when storage is full
        if free_space == 0 && free_bars != 0 {
            if team.resources.get(&Resource::GOLD).unwrap_or(&0).clone() > 0 && gold_length == 0 {
                gold_length += free_bars;
            } else if team.resources.get(&Resource::SCRAPS).unwrap_or(&0).clone() > 0
                && scraps_length == 0
            {
                scraps_length += free_bars;
            } else if team.resources.get(&Resource::RUM).unwrap_or(&0).clone() > 0
                && rum_length == 0
            {
                rum_length += free_bars;
            } else if gold_length >= scraps_length && gold_length >= rum_length {
                gold_length += free_bars;
            } else if rum_length > gold_length && rum_length >= scraps_length {
                rum_length += free_bars;
            } else if scraps_length > gold_length && scraps_length > rum_length {
                scraps_length += free_bars;
            }
            free_bars = 0
        }

        let home_planet = world.get_planet_or_err(team.home_planet_id)?;
        let asteroid_name = if home_planet.planet_type == PlanetType::Asteroid {
            home_planet.name.clone()
        } else {
            "None".into()
        };

        let info = Paragraph::new(vec![
            Line::from(""),
            Line::from(format!(" Rating {}", world.team_rating(team.id).stars())),
            Line::from(format!(" Reputation {}", team.reputation.stars())),
            Line::from(format!(" Treasury {} {}", team.balance(), CURRENCY_SYMBOL)),
            Line::from(format!(" Asteroid: {}", asteroid_name)),
            Line::from(""),
            Line::from(vec![
                Span::raw(format!(
                    " Storage: {:>4}/{:<4} ",
                    team.used_storage_capacity(),
                    team.max_storage_capacity(),
                )),
                Span::styled("▰".repeat(gold_length), UiStyle::STORAGE_GOLD),
                Span::styled("▰".repeat(scraps_length), UiStyle::STORAGE_SCRAPS),
                Span::styled("▰".repeat(rum_length), UiStyle::STORAGE_RUM),
                Span::raw("▱".repeat(free_bars)),
            ]),
            Line::from(vec![
                Span::styled("    Gold", UiStyle::STORAGE_GOLD),
                Span::raw(format!(
                    ":   {} Kg",
                    team.resources.get(&Resource::GOLD).unwrap_or(&0)
                )),
            ]),
            Line::from(vec![
                Span::styled("    Scraps", UiStyle::STORAGE_SCRAPS),
                Span::raw(format!(
                    ": {} t",
                    team.resources.get(&Resource::SCRAPS).unwrap_or(&0)
                )),
            ]),
            Line::from(vec![
                Span::styled("    Rum", UiStyle::STORAGE_RUM),
                Span::raw(format!(
                    ":    {} l",
                    team.resources.get(&Resource::RUM).unwrap_or(&0)
                )),
            ]),
        ]);

        frame.render_widget(info.block(default_block().title("Info")), split[0]);

        let btm_split = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(
            split[0].inner(&Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );
        let button_split = Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(btm_split[1]);
        if let Ok(go_to_team_current_planet_button) = go_to_team_current_planet_button(
            world,
            team,
            &self.callback_registry,
            hover_text_target,
        ) {
            frame.render_widget(go_to_team_current_planet_button, button_split[0]);
        }

        if let Ok(home_planet_button) =
            go_to_team_home_planet_button(world, team, &self.callback_registry, hover_text_target)
        {
            frame.render_widget(home_planet_button, button_split[1]);
        }

        match team.current_location {
            TeamLocation::OnPlanet { planet_id } => {
                self.render_on_planet_spaceship(frame, world, split[1], planet_id)?
            }
            TeamLocation::Travelling {
                to,
                started,
                duration,
                ..
            } => {
                let countdown = if started + duration > world.last_tick_short_interval {
                    (started + duration - world.last_tick_short_interval).formatted()
                } else {
                    (0 as Tick).formatted()
                };
                self.render_travelling_spaceship(frame, world, split[1], to, countdown)?
            }
            TeamLocation::Exploring {
                around,
                started,
                duration,
                ..
            } => {
                let countdown = if started + duration > world.last_tick_short_interval {
                    (started + duration - world.last_tick_short_interval).formatted()
                } else {
                    (0 as Tick).formatted()
                };
                self.render_exploring_spaceship(frame, world, split[1], around, countdown)?
            }
        }
        Ok(())
    }

    fn render_games(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Min(30), Constraint::Max(48)]).split(area);
        self.render_recent_games(frame, world, split[0])?;
        self.render_challenge_teams(frame, world, split[1])?;
        Ok(())
    }

    fn render_challenge_teams(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Min(16), Constraint::Max(24)]).split(
            area.inner(&Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        let left_split =
            Layout::vertical([Constraint::Length(3)].repeat(self.challenge_teams.len()))
                .split(split[0]);
        let right_split =
            Layout::vertical([Constraint::Length(3)].repeat(self.challenge_teams.len()))
                .split(split[1]);

        let hover_text_target = hover_text_target(&frame);

        for (idx, &team_id) in self.challenge_teams.iter().enumerate() {
            let team = world.get_team_or_err(team_id)?;

            frame.render_widget(
                Paragraph::new(format!(
                    "{:<14} {}",
                    team.name,
                    world.team_rating(team_id).stars()
                )),
                left_split[idx].inner(&Margin {
                    horizontal: 1,
                    vertical: 1,
                }),
            );

            let challenge_button = challenge_button(
                world,
                team,
                &self.callback_registry,
                hover_text_target,
                idx == 0,
            )?;

            frame.render_widget(challenge_button, right_split[idx]);
        }

        frame.render_widget(default_block().title("Open to challenge"), area);

        Ok(())
    }

    fn render_recent_games(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        frame.render_widget(default_block().title("Recent Games".to_string()), area);

        if self.recent_games.len() == 0 {
            return Ok(());
        }

        let team = world.get_own_team()?;
        let split = Layout::horizontal([Constraint::Max(36), Constraint::Min(20)]).split(area);

        let mut options = vec![];
        if team.current_game.is_some() {
            if let Some(game) = world.games.get(&team.current_game.unwrap()) {
                if let Some(action) = game.action_results.last() {
                    let text = format!(
                        " {:>12} {:>3}-{:<3} {:<}",
                        game.home_team_in_game.name,
                        action.home_score,
                        action.away_score,
                        game.away_team_in_game.name,
                    );
                    let style = UiStyle::OWN_TEAM;
                    options.push((text, style));
                }
            }
        }

        for game in world.past_games.values() {
            let text = format!(
                " {:>12} {:>3}-{:<3} {:<}",
                game.home_team_name, game.home_score, game.away_score, game.away_team_name,
            );

            let style = UiStyle::DEFAULT;
            options.push((text, style));
        }
        let list = selectable_list(options, &self.callback_registry);

        frame.render_stateful_widget(
            list,
            split[0].inner(&Margin {
                horizontal: 1,
                vertical: 1,
            }),
            &mut ClickableListState::default().with_selected(self.game_index),
        );

        let game_id = self.recent_games[self.game_index.unwrap()];
        let summary = if let Ok(current_game) = world.get_game_or_err(game_id) {
            Paragraph::new(format!(
                "Location {} - Attendance {}\nCurrently playing: {}",
                world.get_planet_or_err(current_game.location)?.name,
                current_game.attendance,
                current_game.timer.format(),
            ))
        } else {
            if self.loaded_games.get(&game_id).is_none() {
                let game =
                    load_from_json(format!("{}{}.json", PERSISTED_GAMES_PREFIX, game_id).as_str())?;
                self.loaded_games.insert(game_id, game);
            }
            let game = world
                .past_games
                .get(&game_id)
                .ok_or("Unable to get past game.")?;

            let loaded_game = self
                .loaded_games
                .get(&game_id)
                .expect("Failed to load game");

            let home_mvps = loaded_game
                .home_team_mvps
                .as_ref()
                .expect("Loaded game should have set mvps.");
            let away_mvps = loaded_game
                .away_team_mvps
                .as_ref()
                .expect("Loaded game should have set mvps.");

            let lines = vec![
                Line::from(format!(
                    "Location {} - Attendance {}",
                    world.get_planet_or_err(game.location)?.name,
                    game.attendance
                )),
                Line::from(format!(
                    "Ended on {}",
                    game.ended_at
                        .expect("Past games should have ended")
                        .formatted_as_date()
                )),
                Line::from(""),
                Line::from(""),
                Line::from(Span::styled(game.home_team_name.clone(), UiStyle::HEADER)),
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    home_mvps[0].name,
                    format!(
                        "{:>2} {}",
                        home_mvps[0].best_stats[0].1, home_mvps[0].best_stats[0].0
                    ),
                    format!(
                        "{:>2} {}",
                        home_mvps[0].best_stats[1].1, home_mvps[0].best_stats[1].0
                    ),
                    format!(
                        "{:>2} {}",
                        home_mvps[0].best_stats[2].1, home_mvps[0].best_stats[2].0
                    )
                )),
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    home_mvps[1].name,
                    format!(
                        "{:>2} {}",
                        home_mvps[1].best_stats[0].1, home_mvps[1].best_stats[0].0
                    ),
                    format!(
                        "{:>2} {}",
                        home_mvps[1].best_stats[1].1, home_mvps[1].best_stats[1].0
                    ),
                    format!(
                        "{:>2} {}",
                        home_mvps[1].best_stats[2].1, home_mvps[1].best_stats[2].0
                    )
                )),
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    home_mvps[2].name,
                    format!(
                        "{:>2} {}",
                        home_mvps[2].best_stats[0].1, home_mvps[2].best_stats[0].0
                    ),
                    format!(
                        "{:>2} {}",
                        home_mvps[2].best_stats[1].1, home_mvps[2].best_stats[1].0
                    ),
                    format!(
                        "{:>2} {}",
                        home_mvps[2].best_stats[2].1, home_mvps[2].best_stats[2].0
                    )
                )),
                Line::from(""),
                Line::from(Span::styled(game.away_team_name.clone(), UiStyle::HEADER)),
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    away_mvps[0].name,
                    format!(
                        "{:>2} {}",
                        away_mvps[0].best_stats[0].1, away_mvps[0].best_stats[0].0
                    ),
                    format!(
                        "{:>2} {}",
                        away_mvps[0].best_stats[1].1, away_mvps[0].best_stats[1].0
                    ),
                    format!(
                        "{:>2} {}",
                        away_mvps[0].best_stats[2].1, away_mvps[0].best_stats[2].0
                    )
                )),
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    away_mvps[1].name,
                    format!(
                        "{:>2} {}",
                        away_mvps[1].best_stats[0].1, away_mvps[1].best_stats[0].0
                    ),
                    format!(
                        "{:>2} {}",
                        away_mvps[1].best_stats[1].1, away_mvps[1].best_stats[1].0
                    ),
                    format!(
                        "{:>2} {}",
                        away_mvps[1].best_stats[2].1, away_mvps[1].best_stats[2].0
                    )
                )),
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    away_mvps[2].name,
                    format!(
                        "{:>2} {}",
                        away_mvps[2].best_stats[0].1, away_mvps[2].best_stats[0].0
                    ),
                    format!(
                        "{:>2} {}",
                        away_mvps[2].best_stats[1].1, away_mvps[2].best_stats[1].0
                    ),
                    format!(
                        "{:>2} {}",
                        away_mvps[2].best_stats[2].1, away_mvps[2].best_stats[2].0
                    )
                )),
            ];

            Paragraph::new(lines)
        };

        frame.render_widget(
            summary,
            split[1].inner(&Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        Ok(())
    }

    fn render_player_buttons(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;
        if self.player_index.is_none() {
            return Ok(());
        }
        let player_id = self.players[self.player_index.unwrap()];
        let player = world
            .get_player(player_id)
            .ok_or(format!("Player {:?} not found", player_id).to_string())?;
        let hover_text_target = hover_text_target(frame);
        let button_splits = Layout::horizontal([
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(32),
            Constraint::Length(32),
            Constraint::Min(1),
        ])
        .split(area.inner(&Margin {
            vertical: 0,
            horizontal: 1,
        }));
        let can_set_as_captain = team.can_set_crew_role(&player, CrewRole::Captain);
        let percentage = {
            let bonus = world.team_reputation_bonus(Some(player_id))?;
            ((bonus - BASE_BONUS) * 100.0) as u32
        };
        let mut captain_button = Button::new(
            "captain".into(),
            UiCallbackPreset::SetCrewRole {
                player_id,
                role: CrewRole::Captain,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text(
            format!(
                "Set player to captain role. The team reputation update bonus would be {}%",
                percentage
            ),
            hover_text_target,
        )
        .set_hotkey(UiKey::SET_CAPTAIN);
        if can_set_as_captain.is_err() {
            captain_button.disable(None);
        }
        frame.render_widget(captain_button, button_splits[0]);

        let can_set_as_pilot = team.can_set_crew_role(&player, CrewRole::Pilot);
        let percentage = {
            let bonus = world.spaceship_speed_bonus(Some(player_id))?;
            ((bonus - BASE_BONUS) * 100.0) as u32
        };

        let mut pilot_button = Button::new(
            "pilot".into(),
            UiCallbackPreset::SetCrewRole {
                player_id,
                role: CrewRole::Pilot,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text(
            format!(
                "Set player to pilot role. The spaceship speed would increase by {}%",
                percentage
            ),
            hover_text_target,
        )
        .set_hotkey(UiKey::SET_PILOT);
        if can_set_as_pilot.is_err() {
            pilot_button.disable(None);
        }
        frame.render_widget(pilot_button, button_splits[1]);

        let can_set_as_doctor = team.can_set_crew_role(&player, CrewRole::Doctor);
        let percentage = {
            let bonus = world.tiredness_recovery_bonus(Some(player_id))?;
            ((bonus - BASE_BONUS) * 100.0) as u32
        };
        let mut doctor_button = Button::new(
            "doctor".into(),
            UiCallbackPreset::SetCrewRole {
                player_id,
                role: CrewRole::Doctor,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text(
            format!(
                "Set player to doctor role. The team tiredness recovery bonus would be {}%",
                percentage
            ),
            hover_text_target,
        )
        .set_hotkey(UiKey::SET_DOCTOR);
        if can_set_as_doctor.is_err() {
            doctor_button.disable(None);
        }
        frame.render_widget(doctor_button, button_splits[2]);

        let can_release = team.can_release_player(&player);
        let mut release_button = Button::new(
            format!(
                "Fire {}.{}",
                player.info.first_name.chars().next().unwrap_or_default(),
                player.info.last_name
            ),
            UiCallbackPreset::ReleasePlayer { player_id },
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text("Fire pirate from the crew!".into(), hover_text_target)
        .set_hotkey(UiKey::FIRE);
        if can_release.is_err() {
            release_button.disable(Some(format!("{}", can_release.unwrap_err().to_string())));
        }

        frame.render_widget(release_button, button_splits[3]);

        let can_change_training_focus = team.can_change_training_focus();
        let mut training_button = Button::new(
            format!(
                "Training focus: {}",
                if let Some(focus) = player.training_focus {
                    focus.to_string()
                } else {
                    "General".to_string()
                }
            ),
            UiCallbackPreset::NextTrainingFocus { player_id },
            Arc::clone(&self.callback_registry),
        ).set_hover_text(
                "Change the training focus, which affects which skills will increase more rapidly after a game.".into(),
            hover_text_target,
        )
        .set_hotkey(UiKey::TRAINING_FOCUS);
        if can_change_training_focus.is_err() {
            training_button.disable(Some(format!(
                "{}",
                can_change_training_focus.unwrap_err().to_string()
            )));
        }
        frame.render_widget(training_button, button_splits[4]);

        Ok(())
    }

    fn build_players_table(&self, world: &World) -> AppResult<ClickableTable> {
        let team = world.get_own_team().unwrap();
        let header_cells = [" Name", "Training", "Current", "Best", "Role", "Crew bonus"]
            .iter()
            .map(|h| ClickableCell::from(*h).style(UiStyle::HEADER));
        let header = ClickableRow::new(header_cells);
        let rows = self
            .players
            .iter()
            .map(|&id| {
                let player = world.get_player(id).unwrap();
                let skills = player.current_skill_array();

                let training_focus = if let Some(focus) = player.training_focus {
                    focus.to_string()
                } else {
                    "General".to_string()
                };

                let current_role = match team.player_ids.iter().position(|id| *id == player.id) {
                    Some(idx) => format!(
                        "{:<2} {:<5}",
                        (idx as Position).as_str(),
                        if (idx as Position) < MAX_POSITION {
                            (idx as Position).player_rating(skills).stars()
                        } else {
                            "".to_string()
                        }
                    ),
                    None => "Free agent".to_string(),
                };
                let best_role = Position::best(skills);

                let bonus_string = match player.info.crew_role {
                    CrewRole::Pilot => {
                        let bonus = world.spaceship_speed_bonus(team.crew_roles.pilot)?;
                        let fitness = ((bonus - BASE_BONUS) / BONUS_PER_SKILL).bound();
                        let style = fitness.style();
                        let percentage = ((bonus - BASE_BONUS) * 100.0) as u32;
                        Span::styled(format!("Ship speed +{}%", percentage), style)
                    }
                    CrewRole::Captain => {
                        let bonus = world.team_reputation_bonus(team.crew_roles.captain)?;
                        let fitness = ((bonus - BASE_BONUS) / BONUS_PER_SKILL).bound();
                        let style = fitness.style();
                        let percentage = ((bonus - BASE_BONUS) * 100.0) as u32;
                        Span::styled(format!("Reputation +{}%", percentage), style)
                    }
                    CrewRole::Doctor => {
                        let bonus = world.tiredness_recovery_bonus(team.crew_roles.doctor)?;
                        let fitness = ((bonus - BASE_BONUS) / BONUS_PER_SKILL).bound();
                        let style = fitness.style();
                        let percentage = ((bonus - BASE_BONUS) * 100.0) as u32;
                        Span::styled(format!("Recovery   +{}%", percentage), style)
                    }
                    _ => Span::raw(""),
                };

                let cells = [
                    ClickableCell::from(format!(
                        " {} {}",
                        player.info.first_name, player.info.last_name
                    )),
                    ClickableCell::from(training_focus.to_string()),
                    ClickableCell::from(current_role),
                    ClickableCell::from(format!(
                        "{:<2} {:<5}",
                        best_role.as_str(),
                        best_role.player_rating(skills).stars()
                    )),
                    ClickableCell::from(player.info.crew_role.to_string()),
                    ClickableCell::from(bonus_string),
                ];
                Ok(ClickableRow::new(cells))
            })
            .collect::<AppResult<Vec<ClickableRow>>>();
        let table = ClickableTable::new(rows?, Arc::clone(&self.callback_registry))
            .header(header)
            .hovering_style(UiStyle::HIGHLIGHT)
            .highlight_style(UiStyle::SELECTED)
            .widths(&[
                Constraint::Length(26),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(18),
            ]);

        Ok(table)
    }

    fn render_players_top(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let hover_text_target = hover_text_target(frame);
        let team = world.get_own_team()?;
        let top_split =
            Layout::horizontal([Constraint::Min(10), Constraint::Length(60)]).split(area);

        let table = self.build_players_table(world)?;

        frame.render_stateful_widget(
            table.block(default_block().title(format!(
                "{} {} ↓/↑",
                team.name.clone(),
                world.team_rating(team.id).stars()
            ))),
            top_split[0],
            &mut ClickableTableState::default().with_selected(self.player_index),
        );

        if self.player_index.is_none() {
            return Ok(());
        }
        let player_id = self.players[self.player_index.unwrap()];

        let player = world
            .get_player(player_id)
            .ok_or(format!("Player {:?} not found", player_id).to_string())?;

        render_player_description(
            player,
            &self.gif_map,
            &self.callback_registry,
            self.tick,
            frame,
            world,
            top_split[1],
        );

        let table_bottom = Layout::vertical([
            Constraint::Min(10),
            Constraint::Length(3), //role buttons
            Constraint::Length(3), //buttons
            Constraint::Length(1), //margin box
        ])
        .split(area);

        let position_button_splits = Layout::horizontal([
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(3),  //margin
            Constraint::Length(32), //auto-assign
            Constraint::Length(32), //tactic
            Constraint::Min(0),
        ])
        .split(table_bottom[1].inner(&Margin {
            vertical: 0,
            horizontal: 1,
        }));

        for idx in 0..MAX_POSITION as usize {
            let position = idx as Position;
            let rect = position_button_splits[idx];
            let mut button = Button::new(
                format!("{}:{:<2}", (idx + 1), position.as_str()),
                UiCallbackPreset::SwapPlayerPositions {
                    player_id,
                    position: idx,
                },
                Arc::clone(&self.callback_registry),
            )
            .set_hover_text(
                format!("Set player initial position to {}.", position.as_str()),
                hover_text_target,
            )
            .set_hotkey(UiKey::set_player_position(idx as Position));

            let position = team.player_ids.iter().position(|id| *id == player.id);
            if position.is_some() && position.unwrap() == idx {
                button.disable(None);
            }
            frame.render_widget(button, rect);
        }
        let auto_assign_button = Button::new(
            "Auto-assign positions".into(),
            UiCallbackPreset::AssignBestTeamPositions,
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text(
            "Auto-assign players' initial position.".into(),
            hover_text_target,
        )
        .set_hotkey(UiKey::AUTO_ASSIGN);
        frame.render_widget(auto_assign_button, position_button_splits[6]);

        let offense_tactic_button = Button::new(
            format!("tactic: {}", team.game_tactic),
            UiCallbackPreset::SetTeamTactic {
                tactic: team.game_tactic.next(),
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text(
            "Set team tactic. This affects the actions the team will choose during the game."
                .into(),
            hover_text_target,
        )
        .set_hotkey(UiKey::SET_TACTIC);
        frame.render_widget(offense_tactic_button, position_button_splits[7]);

        self.render_player_buttons(frame, world, table_bottom[2])?;
        Ok(())
    }

    fn render_on_planet_spaceship(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
        _planet_id: PlanetId,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;
        let hover_text_target = hover_text_target(&frame);

        let split = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(
            area.inner(&Margin {
                vertical: 1,
                horizontal: 1,
            }),
        );

        render_spaceship_description(&team, &self.gif_map, self.tick, world, frame, area);

        if let Ok(explore_button) =
            explore_button(world, team, &self.callback_registry, hover_text_target)
        {
            frame.render_widget(explore_button, split[1]);
        }
        Ok(())
    }

    fn render_travelling_spaceship(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
        planet_id: PlanetId,
        countdown: String,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;
        if let Ok(mut lines) = self
            .gif_map
            .lock()
            .unwrap()
            .travelling_spaceship_lines(team.id, self.tick, world)
        {
            let rect = area.inner(&Margin {
                horizontal: 1,
                vertical: 1,
            });
            // Apply y-centering
            let min_offset = if lines.len() > rect.height as usize {
                (lines.len() - rect.height as usize) / 2
            } else {
                0
            };
            let max_offset = lines.len().min(min_offset + rect.height as usize);
            if min_offset > 0 || max_offset < lines.len() {
                lines = lines[min_offset..max_offset].to_vec();
            }
            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph.centered(), rect);
        }
        let planet = world.get_planet_or_err(planet_id)?;
        frame.render_widget(
            default_block().title(format!("Travelling to {} - {}", planet.name, countdown)),
            area,
        );
        Ok(())
    }

    fn render_exploring_spaceship(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
        planet_id: PlanetId,
        countdown: String,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;
        if let Ok(mut lines) = self
            .gif_map
            .lock()
            .unwrap()
            .exploring_spaceship_lines(team.id, self.tick, world)
        {
            let rect = area.inner(&Margin {
                horizontal: 1,
                vertical: 1,
            });
            // Apply y-centering
            let min_offset = if lines.len() > rect.height as usize {
                (lines.len() - rect.height as usize) / 2
            } else {
                0
            };
            let max_offset = lines.len().min(min_offset + rect.height as usize);
            if min_offset > 0 || max_offset < lines.len() {
                lines = lines[min_offset..max_offset].to_vec();
            }
            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph.centered(), rect);
        }
        let planet = world.get_planet_or_err(planet_id)?;
        frame.render_widget(
            default_block().title(format!("Exploring around {} - {}", planet.name, countdown)),
            area,
        );
        Ok(())
    }

    pub fn set_view(&mut self, view: MyTeamView) {
        self.view = view;
    }

    pub fn reset_view(&mut self) {
        self.set_view(MyTeamView::Info);
    }
}

impl Screen for MyTeamPanel {
    fn name(&self) -> &str {
        "My Team"
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;
        self.own_team_id = world.own_team_id;

        self.current_planet_id = match world.get_own_team()?.current_location {
            TeamLocation::OnPlanet { planet_id } => Some(planet_id),
            _ => None,
        };

        if self.players.len() != world.get_own_team()?.player_ids.len() || world.dirty_ui {
            let own_team = world.get_own_team()?;
            self.players = own_team.player_ids.clone();
            self.players.sort_by(|a, b| {
                let a = world.get_player(*a).unwrap();
                let b = world.get_player(*b).unwrap();
                if a.rating() == b.rating() {
                    b.total_skills().cmp(&a.total_skills())
                } else {
                    b.rating().cmp(&a.rating())
                }
            });
        }

        if self.planet_markets.len() == 0 || world.dirty_ui {
            self.planet_markets = world
                .planets
                .iter()
                .filter(|(_, planet)| planet.total_population() > 0)
                .sorted_by(|(_, a), (_, b)| b.total_population().cmp(&a.total_population()))
                .map(|(id, _)| id.clone())
                .collect::<Vec<PlanetId>>();
            if self.planet_index.is_none() && self.planet_markets.len() > 0 {
                self.planet_index = Some(0);
            }
        }

        self.player_index = if self.players.len() > 0 {
            if let Some(index) = self.player_index {
                Some(index % self.players.len())
            } else {
                Some(0)
            }
        } else {
            None
        };

        if world.dirty_ui {
            let own_team = world.get_own_team()?;
            let mut games = vec![];
            if let Some(current_game) = own_team.current_game {
                games.push(current_game);
            }

            for game in world.past_games.values() {
                games.push(game.id);
            }
            self.recent_games = games;

            self.challenge_teams = world
                .teams
                .keys()
                .into_iter()
                .filter(|&&id| {
                    let team = world.get_team_or_err(id).unwrap();
                    team.can_challenge_team(own_team).is_ok()
                })
                .cloned()
                .collect();
            self.challenge_teams.sort_by(|a, b| {
                let a = world.get_team_or_err(*a).unwrap();
                let b = world.get_team_or_err(*b).unwrap();
                world
                    .team_rating(b.id)
                    .partial_cmp(&world.team_rating(a.id))
                    .unwrap()
            });
        }

        self.game_index = if self.recent_games.len() > 0 {
            if let Some(index) = self.game_index {
                Some(index % self.recent_games.len())
            } else {
                Some(0)
            }
        } else {
            None
        };

        Ok(())
    }

    fn render(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::vertical([Constraint::Length(24), Constraint::Min(8)]).split(area);

        if self.callback_registry.lock().unwrap().is_hovering(split[0]) {
            self.active_list = PanelList::Top;
        } else {
            self.active_list = PanelList::Bottom;
        }

        self.render_players_top(frame, world, split[0])?;

        let bottom_split =
            Layout::horizontal([Constraint::Length(32), Constraint::Min(40)]).split(split[1]);

        self.render_view_buttons(frame, bottom_split[0]);

        match self.view {
            MyTeamView::Info => self.render_info(frame, world, bottom_split[1])?,
            MyTeamView::Games => self.render_games(frame, world, bottom_split[1])?,
            MyTeamView::Market => self.render_market(frame, world, bottom_split[1])?,
        }

        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        world: &World,
    ) -> Option<UiCallbackPreset> {
        if self.players.is_empty() {
            return None;
        }
        match key_event.code {
            KeyCode::Up => {
                self.next_index();
            }
            KeyCode::Down => {
                self.previous_index();
            }

            UiKey::CYCLE_VIEW => {
                return Some(UiCallbackPreset::SetMyTeamPanelView {
                    view: self.view.next(),
                });
            }

            UiKey::BUY_SCRAPS => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(buy_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_buy_price(Resource::SCRAPS))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::SCRAPS,
                            amount: 1,
                            unit_cost: buy_price,
                        });
                    }
                }
            }

            UiKey::BUY_GOLD => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(buy_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_buy_price(Resource::GOLD))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::SCRAPS,
                            amount: 1,
                            unit_cost: buy_price,
                        });
                    }
                }
            }

            UiKey::BUY_FUEL => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(buy_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_buy_price(Resource::FUEL))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::FUEL,
                            amount: 1,
                            unit_cost: buy_price,
                        });
                    }
                }
            }

            UiKey::BUY_RUM => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(buy_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_buy_price(Resource::RUM))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::RUM,
                            amount: 1,
                            unit_cost: buy_price,
                        });
                    }
                }
            }

            UiKey::SELL_SCRAPS => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(sell_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_sell_price(Resource::SCRAPS))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::SCRAPS,
                            amount: -1,
                            unit_cost: sell_price,
                        });
                    }
                }
            }

            UiKey::SELL_GOLD => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(sell_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_sell_price(Resource::GOLD))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::GOLD,
                            amount: -1,
                            unit_cost: sell_price,
                        });
                    }
                }
            }

            UiKey::SELL_FUEL => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(sell_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_sell_price(Resource::FUEL))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::FUEL,
                            amount: -1,
                            unit_cost: sell_price,
                        });
                    }
                }
            }

            UiKey::SELL_RUM => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(sell_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_sell_price(Resource::RUM))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::RUM,
                            amount: -1,
                            unit_cost: sell_price,
                        });
                    }
                }
            }

            _ => {}
        }

        None
    }

    fn footer_spans(&self) -> Vec<Span> {
        vec![]
    }
}

impl SplitPanel for MyTeamPanel {
    fn index(&self) -> usize {
        if self.active_list == PanelList::Bottom && self.view == MyTeamView::Games {
            return self.game_index.unwrap_or_default();
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Market {
            return self.planet_index.unwrap_or_default();
        }

        // we should always have at least 1 player
        self.player_index.unwrap_or_default()
    }

    fn max_index(&self) -> usize {
        if self.active_list == PanelList::Bottom && self.view == MyTeamView::Games {
            return self.recent_games.len();
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Market {
            return self.planet_markets.len();
        }
        self.players.len()
    }

    fn set_index(&mut self, index: usize) {
        if self.max_index() == 0 {
            if self.active_list == PanelList::Bottom && self.view == MyTeamView::Games {
                self.game_index = None;
            } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Market {
                self.planet_index = None;
            } else {
                self.player_index = None;
            }
        } else {
            if self.active_list == PanelList::Bottom && self.view == MyTeamView::Games {
                self.game_index = Some(index % self.max_index());
            } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Market {
                self.planet_index = Some(index % self.max_index());
            } else {
                self.player_index = Some(index % self.max_index());
            }
        }
    }
}
